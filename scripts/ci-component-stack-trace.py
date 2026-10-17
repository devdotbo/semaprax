#!/usr/bin/env python3
"""Trace the exact failing result-v3 test on its owning hosted Linux runner."""
import hashlib
import json
import os
from pathlib import Path
import re
import runpy
import shutil
import subprocess
import sys

ROOT = Path(__file__).resolve().parent.parent
CASE = "wit_component::result_v3::tests::deterministic_result_component_is_exactly_parsed_and_upstream_valid"
SUPPORT = runpy.run_path(str(ROOT / "scripts/ci-low-stack-trace.py"))
select_executable = SUPPORT["select_executable"]
elf_tls_layout = SUPPORT["elf_tls_layout"]
FRAME_LAYOUT_CODE = SUPPORT["FRAME_LAYOUT_CODE"]


def component_test_command():
    # Keep the owning script's root-manifest/default-feature build. The unit
    # shard's workspace/all-features profile would select a different binary.
    return ["cargo", "test", "--locked", "--offline", "--manifest-path",
            str(ROOT / "Cargo.toml"), "--features", "unstable-wit-component-harness",
            "--lib", "--no-run", "--message-format=json"]


def main():
    if sys.platform != "linux" or os.environ.get("CI") != "true":
        raise ValueError("the stack trace runs only on the owning hosted Linux CI runner")
    expected = os.environ.get("GITHUB_SHA", "")
    current = subprocess.check_output(["git", "rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    if not re.fullmatch(r"[0-9a-f]{40}", expected) or current != expected:
        raise ValueError("stack trace checkout differs from the current CI commit")
    debugger = shutil.which("gdb")
    if debugger is None:
        raise ValueError("the Linux runner has no gdb; no stack trace was captured")
    router = runpy.run_path(str(ROOT / "scripts/ci-msrv.py"))
    environment = router["cargo_environment"]()
    # Match the owning Component job's exact feature set. This only asks Cargo
    # for its current artifact records; it neither substitutes a saved executable
    # nor changes the owning shard's command, flags, result or test inventory.
    messages = router["capture_listing"](
        component_test_command(),
        environment,
    )
    target = Path(environment.get("CARGO_TARGET_DIR", ROOT / "target"))
    if not target.is_absolute():
        target = ROOT / target
    executable = select_executable(messages, ROOT, target)
    listing = router["parse_listing"](router["capture_listing"](
        [str(executable), CASE, "--exact", "--list"], environment,
    ))
    ignored = router["parse_listing"](router["capture_listing"](
        [str(executable), CASE, "--exact", "--list", "--ignored"], environment,
    ))
    if listing != [CASE] or ignored:
        raise ValueError("the current executable does not contain the exact runnable stack case")
    digest = hashlib.sha256()
    with executable.open("rb") as stream:
        for chunk in iter(lambda: stream.read(64 * 1024), b""):
            digest.update(chunk)
    print(json.dumps({
        "diagnostic_only": True, "head": current, "case": CASE,
        "executable": str(executable.relative_to(ROOT)),
        "executable_sha256": digest.hexdigest(),
        "qualification": "owning Component gate failure remains authoritative",
        "cargo_command": component_test_command(),
    }, sort_keys=True), flush=True)
    print(json.dumps({
        "diagnostic_only": True, "schema": "semaprax.elf-tls.v1", "head": current,
        "tls": elf_tls_layout(executable),
    }, sort_keys=True), flush=True)
    commands = [
        "set pagination off", "set print thread-events off",
        "set print frame-arguments none", "set auto-load off",
        "set debuginfod enabled off", "set disable-randomization off",
        "handle SIGPIPE nostop noprint pass", "handle SIGSEGV stop print nopass",
        "handle SIGBUS stop print nopass", "handle SIGABRT stop print nopass",
        "run", "python exec(" + repr(FRAME_LAYOUT_CODE) + ")", "thread apply all bt 48",
    ]
    command = [debugger, "--batch", "--nx"]
    for instruction in commands:
        command.extend(["-ex", instruction])
    command.extend(["--args", str(executable), CASE, "--exact", "--nocapture", "--test-threads=1"])
    # Retain the original test thread settings and stop at the first fault.
    # Diagnostic execution cannot qualify or replace the owning failed gate.
    return subprocess.run(command, cwd=ROOT, env=environment, timeout=180).returncode


if __name__ == "__main__":
    raise SystemExit(main())
