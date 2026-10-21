import json
import sys
import tempfile
import time
import unittest
from pathlib import Path
from unittest.mock import patch

import campaign_resources as resources


class CampaignResourceTests(unittest.TestCase):
    def setUp(self):
        # Keep the original disk regressions deterministic on a busy host.
        # New tests separately exercise unavailable/contaminated host coverage.
        patcher = patch.object(resources, "host_snapshot", side_effect=healthy_host)
        patcher.start()
        self.addCleanup(patcher.stop)

    def volumes(self, free=None):
        return [{"device": 1, "path": "/fixture", "free_bytes": free if free is not None else resources.MIN_FREE_BYTES * 2}]

    def test_low_disk_refuses_before_invoking_either_arm(self):
        called = []
        @resources.guarded_attempt
        def launch(repo, artifacts, commit, trial):
            called.append(trial)
            return trial
        with tempfile.TemporaryDirectory() as directory, patch.object(resources, "snapshot", return_value=self.volumes(1)):
            for arm in ("semaprax", "typescript"):
                with self.assertRaisesRegex(ValueError, "paid launch refused"):
                    launch(None, Path(directory), None, {"arm": arm, "number": 1})
        self.assertEqual(called, [])

    def test_mid_attempt_disk_loss_preserves_acceptance_and_cost(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(resources, "snapshot", return_value=self.volumes()):
            monitor = resources.Monitor(Path(directory), "semaprax-01")
            with patch.object(resources, "snapshot", return_value=self.volumes(1)):
                row = monitor.finish({"status": "accepted", "list_price": {"estimated": 2}})
            self.assertEqual(row["status"], "accepted")
            self.assertEqual(row["list_price"], {"estimated": 2})
            self.assertTrue(row["resource_assessment"]["contaminated"])
            self.assertFalse(row["resource_assessment"]["clean_comparison_eligible"])
            self.assertFalse(monitor.reserve.exists())
            self.assertEqual(json.loads(monitor.receipt.read_text())["row"], row)

    def test_transient_enospc_stderr_disqualifies_even_with_restored_free_space(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(resources, "snapshot", return_value=self.volumes()):
            artifacts = Path(directory)
            (artifacts / "transcripts").mkdir()
            (artifacts / "transcripts/typescript-01.stderr.txt").write_text("rollout: No space left on device\n")
            monitor = resources.Monitor(artifacts, "typescript-01")
            row = monitor.finish({"status": "not_accepted"})
            self.assertEqual(row["resource_assessment"]["incidents"][0]["kind"], "disk_error_in_retained_stderr")

    def test_post_launch_io_failure_leaves_emergency_row_and_unknown_usage(self):
        @resources.guarded_attempt
        def launch(repo, artifacts, commit, trial):
            raise OSError(28, "No space left on device")
        with tempfile.TemporaryDirectory() as directory, patch.object(resources, "snapshot", return_value=self.volumes()):
            row = launch(None, Path(directory), None, {"arm": "semaprax", "number": 2})
            receipt = Path(row["resource_assessment"]["receipt"])
            self.assertEqual(json.loads(receipt.read_text())["row"]["number"], 2)
            self.assertTrue(row["runner_error"])
            self.assertEqual(row["observed"], {})
            self.assertEqual(row["list_price"], {})
            self.assertIsNone(row["provider_receipt_actual_usd"])

    def test_monitor_deduplicates_same_volume_and_clean_attempt_keeps_receipt(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory)
            samples = resources.snapshot([path, path / "not-yet-created"])
            self.assertEqual(len(samples), 1)
            with patch.object(resources, "snapshot", return_value=self.volumes()):
                monitor = resources.Monitor(path, "calibration")
                row = monitor.finish({"status": "ready", "separate_from_trials": True})
            self.assertTrue(row["resource_assessment"]["clean_comparison_eligible"])
            self.assertTrue(monitor.receipt.exists())

    def test_post_launch_parse_failure_is_retained_as_unknown_cost(self):
        @resources.guarded_attempt
        def launch(repo, artifacts, commit, trial):
            raise UnicodeDecodeError("utf-8", b"\xff", 0, 1, "damaged transcript")
        with tempfile.TemporaryDirectory() as directory, patch.object(resources, "snapshot", return_value=self.volumes()):
            row = launch(None, Path(directory), None, {"arm": "typescript", "number": 2})
            self.assertEqual(row["status"], "failed")
            self.assertFalse(row["telemetry_valid"])
            self.assertEqual(row["list_price"], {})
            self.assertEqual(json.loads(Path(row["resource_assessment"]["receipt"]).read_text())["row"], row)

    def test_watcher_start_failure_releases_reserve_and_never_invokes_attempt(self):
        called = []
        @resources.guarded_attempt
        def launch(repo, artifacts, commit, trial):
            called.append(trial)
        with tempfile.TemporaryDirectory() as directory, patch.object(resources, "snapshot", return_value=self.volumes()), \
                patch.object(resources.threading.Thread, "start", side_effect=RuntimeError("thread unavailable")):
            row = launch(None, Path(directory), None, {"arm": "semaprax", "number": 1})
            self.assertFalse(list((Path(directory) / "resource-receipts").glob("*.reserve")))
            self.assertTrue(row["runner_error"])
            self.assertEqual(called, [])


def healthy_host():
    now = time.monotonic()
    return {"started_monotonic": now, "observed_monotonic": now, "status": "observed",
            "processes": {"status": "observed"}, "memory": {"status": "observed"},
            "policy_violations": []}


class HostResourceTests(unittest.TestCase):
    def test_process_census_owns_descendants_not_unrelated_heavy_jobs(self):
        raw = b"10 1 10 20 /usr/bin/python3\n11 10 10 30 /bin/cargo\n12 11 10 40 /bin/rustc\n20 1 20 50 /other/rustc\n"
        seen = resources.process_observation(raw, 10)
        self.assertEqual(seen["owned_process_count"], 3)
        self.assertEqual(seen["owned_reported_rss_bytes"], 90 * 1024)
        self.assertEqual(seen["known_external_heavy_count"], 1)
        self.assertEqual(seen["known_external_heavy"][0]["pid"], 20)
        self.assertEqual(seen["known_external_heavy"][0]["executable"], "rustc")
        self.assertNotIn("/other/", json.dumps(seen))

    def test_process_parser_refuses_malformed_duplicate_missing_and_oversized(self):
        for raw in [b"", b"1 0 1 nope ps\n", b"1 0 1 2 ps\n1 0 1 2 ps\n",
                    b"0 0 1 2 ps\n", b"1 0 -1 2 ps\n", b"\xff", b"x" * (resources.PROBE_BYTES + 1)]:
            with self.subTest(raw_length=len(raw)), self.assertRaises((ValueError, UnicodeError)):
                resources.parse_processes(raw)
        with self.assertRaisesRegex(ValueError, "absent"):
            resources.process_observation(b"1 0 1 2 ps\n", 2)

    def test_linux_memory_requires_exact_consistent_available_not_free_substitute(self):
        result = resources.parse_linux_memory(b"MemTotal: 4000000 kB\nMemFree: 1 kB\nMemAvailable: 3000000 kB\n")
        self.assertEqual(result["available_bytes"], 3000000 * 1024)
        for raw in [b"MemTotal: 4 kB\nMemFree: 2 kB\n", b"MemTotal: 4 kB\nMemAvailable: 5 kB\n",
                    b"MemTotal: 4 kB\nMemAvailable: 2 kB\nMemAvailable: 2 kB\n", b"MemTotal: -1 kB\n"]:
            with self.assertRaises(ValueError):
                resources.parse_linux_memory(raw)

    def test_darwin_free_percentage_is_not_a_pressure_label(self):
        result = resources.parse_darwin_memory(b"Other counters\nSystem-wide memory free percentage: 42%\n")
        self.assertEqual(result["free_percent"], 42)
        self.assertNotIn("pressure_state", result)
        for raw in [b"normal", b"System-wide memory free percentage: 101%",
                    b"System-wide memory free percentage: 42%\nSystem-wide memory free percentage: 43%\n"]:
            with self.assertRaises(ValueError):
                resources.parse_darwin_memory(raw)

    def test_owned_probe_output_exit_and_timeout_bounds(self):
        self.assertEqual(resources.bounded_probe([sys.executable, "-c", "print('ok')"]), b"ok\n")
        with self.assertRaises(ValueError):
            resources.bounded_probe([sys.executable, "-c", "raise SystemExit(7)"])
        with patch.object(resources, "PROBE_BYTES", 64), self.assertRaises(ValueError):
            resources.bounded_probe([sys.executable, "-c", "print('x' * 65)"])
        with patch.object(resources, "PROBE_TIMEOUT_SECONDS", 0.1), self.assertRaises(ValueError):
            resources.bounded_probe([sys.executable, "-c", "import time; time.sleep(10)"])

    def test_probe_failure_and_unsupported_host_are_unknown(self):
        with patch.object(resources.sys, "platform", "unsupported"), \
                patch.object(resources, "bounded_probe", side_effect=OSError("missing")):
            result = resources.host_snapshot()
        self.assertEqual(result["status"], "unknown")
        self.assertEqual(result["memory"]["reason"], "unsupported platform")
        self.assertEqual(result["processes"]["status"], "unknown")
        self.assertEqual(result["policy_violations"], [])

    def test_known_external_job_and_memory_limits_are_admission_observations(self):
        for raw, percent, expected in [
                (b"10 1 10 1 python\n20 1 20 2 cargo\n", 50, "known_external_heavy_jobs"),
                (b"10 1 10 1 python\n", 1, "memory_headroom_below_policy"),
                (f"10 1 10 {resources.OWNED_RSS_LIMIT_BYTES // 1024 + 1} python\n".encode(), 50,
                 "owned_reported_rss_above_policy")]:
            with self.subTest(expected=expected), patch.object(resources.os, "getpid", return_value=10), \
                    patch.object(resources.sys, "platform", "darwin"), \
                    patch.object(resources, "bounded_probe", side_effect=[raw, f"System-wide memory free percentage: {percent}%\n".encode()]):
                self.assertIn(expected, resources.host_snapshot()["policy_violations"])

    def test_initial_host_refusal_retains_receipt_without_launch_or_reserve(self):
        called = []
        @resources.guarded_attempt
        def launch(repo, artifacts, commit, trial):
            called.append(trial)
        sample = healthy_host()
        sample["policy_violations"] = ["known_external_heavy_jobs"]
        with tempfile.TemporaryDirectory() as directory, \
                patch.object(resources, "snapshot", return_value=[{"device": 1, "path": "/fixture", "free_bytes": resources.MIN_FREE_BYTES * 2}]), \
                patch.object(resources, "host_snapshot", return_value=sample):
            with self.assertRaisesRegex(ValueError, "paid launch refused"):
                launch(None, Path(directory), None, {"arm": "semaprax", "number": 1})
            receipt = json.loads((Path(directory) / "resource-receipts/semaprax-01.json").read_text())
            self.assertEqual(receipt["state"], "refused_host_resources")
            self.assertEqual(receipt["host_samples"], [sample])
            self.assertFalse(list(Path(directory).rglob("*.reserve")))
        self.assertEqual(called, [])

    def test_missing_host_data_keeps_disk_eligible_but_never_clean(self):
        with tempfile.TemporaryDirectory() as directory, \
                patch.object(resources, "snapshot", return_value=[{"device": 1, "path": "/fixture", "free_bytes": resources.MIN_FREE_BYTES * 2}]), \
                patch.object(resources, "host_snapshot", side_effect=lambda: dict(healthy_host(), status="unknown")):
            monitor = resources.Monitor(Path(directory), "typescript-01")
            row = monitor.finish({"status": "accepted", "list_price": {"estimated": 2}})
            self.assertTrue(row["resource_assessment"]["disk_eligible"])
            self.assertFalse(row["resource_assessment"]["clean_comparison_eligible"])
            self.assertEqual(row["resource_assessment"]["host_coverage"]["status"], "unknown")
            self.assertEqual(row["status"], "accepted")
            self.assertEqual(row["list_price"], {"estimated": 2})

    def test_observed_intervals_gaps_bounds_and_clock_drift(self):
        samples = [dict(healthy_host(), started_monotonic=0., observed_monotonic=1.),
                   dict(healthy_host(), started_monotonic=4., observed_monotonic=5.)]
        result = resources.host_coverage(samples, 0., 5., 0)
        self.assertTrue(result["sampled_policy_eligible"])
        self.assertEqual(result["observed_intervals_seconds"], [4.])
        self.assertFalse(result["causal_isolation_established"])
        for changed, ended, omitted in [
                ([samples[0], dict(samples[1], started_monotonic=29., observed_monotonic=30.)], 30., 0),
                (samples, 5., 1), (list(reversed(samples)), 5., 0),
                ([dict(samples[0], observed_monotonic=float("nan")), samples[1]], 5., 0),
                ([samples[0], dict(samples[1], started_monotonic=8.)], 5., 0)]:
            self.assertFalse(resources.host_coverage(changed, 0., ended, omitted)["sampled_policy_eligible"])

    def test_later_observed_pressure_keeps_acceptance_and_cost(self):
        samples = [healthy_host(), dict(healthy_host(), policy_violations=["memory_headroom_below_policy"])]
        with tempfile.TemporaryDirectory() as directory, \
                patch.object(resources, "snapshot", return_value=[{"device": 1, "path": "/fixture", "free_bytes": resources.MIN_FREE_BYTES * 2}]), \
                patch.object(resources, "host_snapshot", side_effect=samples):
            monitor = resources.Monitor(Path(directory), "semaprax-01")
            row = monitor.finish({"status": "accepted", "list_price": {"estimated": 3}})
            self.assertEqual(row["status"], "accepted")
            self.assertEqual(row["list_price"], {"estimated": 3})
            self.assertTrue(row["resource_assessment"]["contaminated"])
            self.assertTrue(row["resource_assessment"]["disk_eligible"])
            self.assertFalse(row["resource_assessment"]["clean_comparison_eligible"])

    def test_sample_bound_retains_counts_and_disqualifies_incomplete_coverage(self):
        with tempfile.TemporaryDirectory() as directory, patch.object(resources, "MAX_HOST_SAMPLES", 2), \
                patch.object(resources, "snapshot", return_value=[{"device": 1, "path": "/fixture", "free_bytes": resources.MIN_FREE_BYTES * 2}]), \
                patch.object(resources, "host_snapshot", side_effect=healthy_host) as probe:
            monitor = resources.Monitor(Path(directory), "typescript-01")
            monitor.observe_host()
            monitor.observe_host()
            row = monitor.finish({"status": "accepted"})
            self.assertEqual(probe.call_count, 2)
            self.assertEqual(len(monitor.host_samples), 2)
            self.assertEqual(row["resource_assessment"]["host_coverage"]["samples_omitted"], 2)
            self.assertIn("sample_bound_exceeded", row["resource_assessment"]["host_coverage"]["coverage_reasons"])
            self.assertFalse(row["resource_assessment"]["clean_comparison_eligible"])


if __name__ == "__main__":
    unittest.main()
