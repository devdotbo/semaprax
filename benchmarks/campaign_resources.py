"""Symmetric disk admission and incident receipts for local paid campaigns.

Headroom is an operational floor, not a guarantee that another task cannot fill
the shared volume. Incidents preserve acceptance and cost denominators but
disqualify the campaign from a clean comparative headline.
"""
from __future__ import annotations

import functools
import hashlib
import json
import math
import os
import re
import selectors
import shutil
import subprocess
import sys
import threading
import time
from pathlib import Path

MIN_FREE_BYTES = 5 * 1024**3
SAMPLE_SECONDS = 5
RESERVE_BYTES = 1024**2
DISK_ERRORS = ("no space left on device", "errno=28", "errno 28", "disk full")
PROBE_TIMEOUT_SECONDS = 3
PROBE_BYTES = 1024**2
MAX_SAMPLE_GAP_SECONDS = 15
MAX_HOST_SAMPLES = 4096
OWNED_RSS_LIMIT_BYTES = 8 * 1024**3
MIN_AVAILABLE_BYTES = 2 * 1024**3
MIN_DARWIN_FREE_PERCENT = 10
KNOWN_HEAVY = frozenset({"cargo", "rustc", "clippy-driver", "clang", "clang++",
                         "gcc", "g++", "cc1", "cc1plus", "ld", "lld", "ninja", "make", "cmake"})


def bounded_probe(command: list[str]) -> bytes:
    """Bound a read-only probe; only this owned probe may be killed on failure."""
    process = subprocess.Popen(command, stdin=subprocess.DEVNULL, stdout=subprocess.PIPE,
                               stderr=subprocess.DEVNULL, close_fds=True)
    data = bytearray()
    deadline = time.monotonic() + PROBE_TIMEOUT_SECONDS
    try:
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ)
            while True:
                remaining = deadline - time.monotonic()
                if remaining <= 0 or not selector.select(remaining):
                    raise ValueError("host probe timed out")
                block = os.read(process.stdout.fileno(), min(65536, PROBE_BYTES + 1 - len(data)))
                if not block:
                    break
                data.extend(block)
                if len(data) > PROBE_BYTES:
                    raise ValueError("host probe output exceeds bound")
        if process.wait(timeout=max(0.001, deadline - time.monotonic())) != 0:
            raise ValueError("host probe failed")
        return bytes(data)
    finally:
        if process.poll() is None:
            process.kill()
        process.wait(timeout=PROBE_TIMEOUT_SECONDS)
        process.stdout.close()


def parse_processes(raw: bytes) -> list[dict]:
    """Never request or retain argv, environment, usernames or working paths."""
    if len(raw) > PROBE_BYTES:
        raise ValueError("process census exceeds bound")
    rows, seen = [], set()
    for line in raw.decode("utf-8", errors="strict").splitlines():
        fields = line.split(None, 4)
        if len(fields) != 5 or not all(re.fullmatch(r"\d+", x) for x in fields[:4]):
            raise ValueError("malformed process census")
        pid, ppid, pgid, rss_kib = map(int, fields[:4])
        name = Path(fields[4]).name
        if pid <= 0 or pid in seen or not name or len(name) > 256:
            raise ValueError("invalid or duplicate process identity")
        seen.add(pid)
        rows.append(dict(pid=pid, ppid=ppid, pgid=pgid, rss_bytes=rss_kib * 1024, executable=name))
    if not rows or len(rows) > 65536:
        raise ValueError("empty or oversized process census")
    return rows


def process_observation(raw: bytes, owner_pid: int) -> dict:
    rows = parse_processes(raw)
    if owner_pid not in {row["pid"] for row in rows}:
        raise ValueError("campaign observer absent from process census")
    owned = {owner_pid}
    while True:
        children = {row["pid"] for row in rows if row["ppid"] in owned}
        if children <= owned:
            break
        owned.update(children)
    external = [row for row in rows if row["pid"] not in owned
                and row["executable"].lower() in KNOWN_HEAVY]
    return {"status": "observed", "raw_sha256": hashlib.sha256(raw).hexdigest(),
            "raw_bytes": len(raw), "process_count": len(rows), "observer_pid": owner_pid,
            "owned_process_count": len(owned),
            "owned_reported_rss_bytes": sum(row["rss_bytes"] for row in rows if row["pid"] in owned),
            "known_external_heavy_count": len(external), "known_external_heavy": external[:32],
            "heavy_records_omitted": max(0, len(external) - 32),
            "scope": "visible process snapshot; parent-linked descendants; RSS may count shared pages more than once"}


def parse_linux_memory(raw: bytes) -> dict:
    if len(raw) > PROBE_BYTES:
        raise ValueError("memory snapshot exceeds bound")
    values = {}
    for line in raw.decode("ascii").splitlines():
        if line.startswith(("MemTotal:", "MemAvailable:")):
            match = re.fullmatch(r"(MemTotal|MemAvailable):\s+(\d+) kB", line)
            if not match or match[1] in values:
                raise ValueError("malformed or duplicate memory field")
            values[match[1]] = int(match[2]) * 1024
    if set(values) != {"MemTotal", "MemAvailable"} or not 0 < values["MemTotal"] or not 0 <= values["MemAvailable"] <= values["MemTotal"]:
        raise ValueError("memory availability unavailable or inconsistent")
    return dict(status="observed", source="/proc/meminfo", total_bytes=values["MemTotal"],
                available_bytes=values["MemAvailable"], raw_sha256=hashlib.sha256(raw).hexdigest())


def parse_darwin_memory(raw: bytes) -> dict:
    if len(raw) > PROBE_BYTES:
        raise ValueError("memory snapshot exceeds bound")
    matches = re.findall(rb"(?m)^System-wide memory free percentage:\s*(\d+)%\s*$", raw)
    if len(matches) != 1 or not 0 <= int(matches[0]) <= 100:
        raise ValueError("Darwin memory free percentage unavailable")
    return dict(status="observed", source="memory_pressure -Q", free_percent=int(matches[0]),
                raw_sha256=hashlib.sha256(raw).hexdigest(),
                scope="reported free percentage; not a kernel pressure-state classification")


def host_snapshot() -> dict:
    started = time.monotonic()
    row = {"started_monotonic": started, "platform": sys.platform, "errors": []}
    try:
        row["processes"] = process_observation(
            bounded_probe(["/bin/ps", "-axo", "pid=,ppid=,pgid=,rss=,comm="]), os.getpid())
    except (OSError, ValueError, subprocess.SubprocessError) as error:
        row["processes"] = {"status": "unknown"}
        row["errors"].append("processes:" + type(error).__name__)
    try:
        if sys.platform == "darwin":
            row["memory"] = parse_darwin_memory(bounded_probe(["/usr/bin/memory_pressure", "-Q"]))
        elif sys.platform.startswith("linux"):
            with open("/proc/meminfo", "rb") as source:
                row["memory"] = parse_linux_memory(source.read(PROBE_BYTES + 1))
        else:
            row["memory"] = {"status": "unknown", "reason": "unsupported platform"}
    except (OSError, ValueError, UnicodeError, subprocess.SubprocessError) as error:
        row["memory"] = {"status": "unknown"}
        row["errors"].append("memory:" + type(error).__name__)
    violations = []
    processes, memory = row["processes"], row["memory"]
    if processes.get("known_external_heavy_count", 0):
        violations.append("known_external_heavy_jobs")
    if processes.get("owned_reported_rss_bytes", 0) > OWNED_RSS_LIMIT_BYTES:
        violations.append("owned_reported_rss_above_policy")
    if (memory.get("available_bytes", MIN_AVAILABLE_BYTES) < MIN_AVAILABLE_BYTES
            or memory.get("free_percent", MIN_DARWIN_FREE_PERCENT) < MIN_DARWIN_FREE_PERCENT):
        violations.append("memory_headroom_below_policy")
    row.update(observed_monotonic=time.monotonic(), policy_violations=violations,
               status="observed" if all(x.get("status") == "observed" for x in (processes, memory)) else "unknown")
    return row


def host_coverage(samples: list[dict], started: float, ended: float, omitted: int) -> dict:
    reasons = []
    times = [x.get("observed_monotonic") for x in samples]
    finite = all(type(x) in (int, float) and math.isfinite(x) for x in [started, ended, *times])
    gaps = [b - a for a, b in zip(times, times[1:])] if finite else []
    if not finite or ended < started or any(x < 0 for x in gaps):
        reasons.append("invalid_monotonic_interval")
    if len(samples) < 2 or any(x.get("status") != "observed"
            or x.get("processes", {}).get("status") != "observed"
            or x.get("memory", {}).get("status") != "observed" for x in samples):
        reasons.append("missing_host_observation")
    if finite and any(type(x.get("started_monotonic")) not in (int, float)
            or not math.isfinite(x["started_monotonic"])
            or not 0 <= x["observed_monotonic"] - x["started_monotonic"] <= MAX_SAMPLE_GAP_SECONDS
            for x in samples):
        reasons.append("invalid_probe_interval")
    if omitted:
        reasons.append("sample_bound_exceeded")
    if finite and samples:
        first = samples[0].get("started_monotonic")
        if (type(first) not in (int, float) or not math.isfinite(first) or first < started
                or first - started > MAX_SAMPLE_GAP_SECONDS or ended < times[-1]
                or ended - times[-1] > MAX_SAMPLE_GAP_SECONDS
                or any(gap > MAX_SAMPLE_GAP_SECONDS for gap in gaps)):
            reasons.append("host_sampling_gap")
    violations = sorted({v for x in samples for v in x.get("policy_violations", [])})
    complete = not reasons
    return {"status": "observed_within_sampled_policy" if complete and not violations else "contaminated" if violations else "unknown",
            "sampled_policy_eligible": complete and not violations,
            "sample_count": len(samples), "samples_omitted": omitted,
            "attempt_started_monotonic": started, "attempt_ended_monotonic": ended,
            "observed_intervals_seconds": gaps, "coverage_reasons": reasons,
            "policy_violations": violations, "causal_isolation_established": False,
            "scope": "bounded point samples; transient or unrecognized workloads between samples remain unobserved"}


def existing_parent(path: Path) -> Path:
    while not path.exists():
        path = path.parent
    return path


def snapshot(paths: list[Path]) -> list[dict]:
    volumes = {}
    for path in paths:
        parent = existing_parent(path.resolve())
        device = parent.stat().st_dev
        if device not in volumes:
            volumes[device] = {"device": device, "path": str(parent),
                               "free_bytes": shutil.disk_usage(parent).free}
    return list(volumes.values())


def require_headroom(paths: list[Path]) -> list[dict]:
    samples = snapshot(paths)
    if any(row["free_bytes"] < MIN_FREE_BYTES for row in samples):
        raise ValueError(f"paid launch refused: require {MIN_FREE_BYTES} free bytes on every output/rollout volume")
    return samples


def policy() -> dict:
    return {"minimum_free_bytes": MIN_FREE_BYTES, "sample_seconds": SAMPLE_SECONDS,
            "reserve_bytes_per_attempt": RESERVE_BYTES,
            "scope": "same admission and monitoring for both arms and separate calibration",
            "acceptance_unchanged": True, "contaminated_attempt_costs_retained": True,
            "host_observation": {"schema": "semaprax.campaign-host-observation.v1",
                "probe_timeout_seconds": PROBE_TIMEOUT_SECONDS, "probe_max_bytes": PROBE_BYTES,
                "maximum_sample_gap_seconds": MAX_SAMPLE_GAP_SECONDS,
                "maximum_samples": MAX_HOST_SAMPLES, "owned_reported_rss_limit_bytes": OWNED_RSS_LIMIT_BYTES,
                "linux_minimum_available_bytes": MIN_AVAILABLE_BYTES,
                "darwin_minimum_reported_free_percent": MIN_DARWIN_FREE_PERCENT,
                "known_heavy_executables": sorted(KNOWN_HEAVY),
                "missing_coverage": "unknown, never clean", "causal_isolation_established": False}}


class Monitor:
    def __init__(self, artifacts: Path, label: str):
        self.artifacts, self.label = artifacts, label
        self.started_monotonic = time.monotonic()
        self.ended_monotonic = None
        self.host_samples = []
        self.host_samples_omitted = 0
        self.paths = [artifacts, Path(os.environ.get("CODEX_HOME", str(Path.home() / ".codex")))]
        self.samples = require_headroom(self.paths)
        self.minimum = {row["device"]: row["free_bytes"] for row in self.samples}
        self.incidents: list[dict] = []
        directory = artifacts / "resource-receipts"
        directory.mkdir(parents=True, exist_ok=True)
        self.receipt = directory / f"{label}.json"
        self.reserve = directory / f"{label}.reserve"
        self.observe_host()
        if self.host_samples[0].get("policy_violations"):
            self.write("refused_host_resources")
            raise ValueError(f"paid launch refused: observed host policy violation; inspect {self.receipt}")
        # Write actual bytes rather than a sparse truncate so this reserve can
        # be released to retain a small emergency row when the volume fills.
        with self.reserve.open("xb") as output:
            output.write(bytes(RESERVE_BYTES))
            output.flush()
            os.fsync(output.fileno())
        self.stop = threading.Event()
        self.thread = threading.Thread(target=self._watch, daemon=True)
        try:
            self.write("prepared")
        except BaseException:
            self.reserve.unlink(missing_ok=True)
            raise

    def write(self, state: str, row: dict | None = None):
        document = {"schema": "semaprax.campaign-resource-receipt.v1", "label": self.label,
                    "state": state, "policy": policy(), "initial_volumes": self.samples,
                    "minimum_free_bytes_by_device": self.minimum, "incidents": self.incidents,
                    "paid_attempt_state": "inspect retained transcript; prepared alone does not prove a paid launch",
                    "host_samples": self.host_samples,
                    "host_coverage": host_coverage(self.host_samples, self.started_monotonic,
                                                   self.ended_monotonic or time.monotonic(), self.host_samples_omitted),
                    "row": row}
        temporary = self.receipt.with_suffix(".tmp")
        with temporary.open("w") as output:
            json.dump(document, output, indent=2, sort_keys=True)
            output.write("\n")
            output.flush()
            os.fsync(output.fileno())
        temporary.replace(self.receipt)

    def observe_host(self):
        if len(self.host_samples) < MAX_HOST_SAMPLES:
            self.host_samples.append(host_snapshot())
        else:
            self.host_samples_omitted += 1

    def sample(self):
        for row in snapshot(self.paths):
            self.minimum[row["device"]] = min(self.minimum.get(row["device"], row["free_bytes"]), row["free_bytes"])
            if row["free_bytes"] < MIN_FREE_BYTES and not self.incidents:
                self.reserve.unlink(missing_ok=True)
                self.incidents.append({"kind": "disk_headroom_below_floor", "unix_seconds": time.time(), **row})
                self.write("resource_contaminated")
        self.observe_host()

    def _watch(self):
        while not self.stop.wait(SAMPLE_SECONDS):
            try:
                self.sample()
            except OSError as error:
                self.reserve.unlink(missing_ok=True)
                self.incidents.append({"kind": "resource_monitor_io_error", "error": str(error)})
                break

    def finish(self, row: dict):
        self.stop.set()
        if self.thread.ident is not None:
            self.thread.join()
        try:
            self.sample()
            stderr = self.artifacts / "transcripts" / f"{self.label}.stderr.txt"
            if stderr.is_file():
                # Rollout recorder failures may happen even if a coarse disk
                # sample misses the moment the volume was full.
                with stderr.open(errors="replace") as source:
                    for line in source:
                        if any(value in line.lower() for value in DISK_ERRORS):
                            self.incidents.append({"kind": "disk_error_in_retained_stderr", "evidence": str(stderr)})
                            break
        except OSError as error:
            self.incidents.append({"kind": "resource_finish_io_error", "error": str(error)})
        self.reserve.unlink(missing_ok=True)
        self.ended_monotonic = time.monotonic()
        coverage = host_coverage(self.host_samples, self.started_monotonic,
                                 self.ended_monotonic, self.host_samples_omitted)
        disk_eligible = not self.incidents
        row["resource_assessment"] = {"contaminated": bool(self.incidents) or bool(coverage["policy_violations"]),
            "disk_eligible": disk_eligible,
            "clean_comparison_eligible": disk_eligible and coverage["sampled_policy_eligible"],
            "clean_comparison_basis": "disk and complete declared point-sampling policy; not causal isolation",
            "host_coverage": coverage, "receipt": str(self.receipt),
            "incidents": self.incidents, "minimum_free_bytes_by_device": {str(key): value for key, value in self.minimum.items()},
            "acceptance_unchanged": True, "cost_retained": True}
        self.write("finished", row)
        return row


def guarded_attempt(function):
    """Keep emergency attempt rows outside results.json without rerunning work."""
    @functools.wraps(function)
    def run(repo, artifacts, commit, *args, **kwargs):
        trial = kwargs.get("trial")
        if trial is None and args and isinstance(args[0], dict) and "arm" in args[0]:
            trial = args[0]
        label = f"{trial['arm']}-{trial['number']:02d}" if trial else "calibration"
        monitor = Monitor(Path(artifacts), label)
        try:
            monitor.thread.start()
            row = function(repo, artifacts, commit, *args, **kwargs)
        except Exception as error:
            monitor.reserve.unlink(missing_ok=True)
            monitor.incidents.append({"kind": "attempt_io_error" if isinstance(error, OSError) else "runner_exception",
                                      "error": str(error), "exception_type": type(error).__name__})
            row = {**(trial or {}), "status": "failed", "runner_error": True,
                   "failure": str(error), "workspace_retained_for_review": True,
                   "provider_receipt_actual_usd": None, "observed": {}, "list_price": {},
                   "telemetry_valid": False,
                   "accounting_note": "interrupted attempt: recover retained transcript; missing usage is unknown"}
        except BaseException:
            monitor.stop.set()
            if monitor.thread.ident is not None:
                monitor.thread.join()
            monitor.reserve.unlink(missing_ok=True)
            raise
        return monitor.finish(row)
    return run
