"""The Component diagnostic uses the owning gate's original test artifact."""
import os
from pathlib import Path
import runpy
import shlex
import unittest
from unittest.mock import patch

ROOT = Path(__file__).resolve().parents[2]
TRACE = runpy.run_path(str(ROOT / "scripts/ci-component-stack-trace.py"))


class ComponentStackTrace(unittest.TestCase):
    def test_build_matches_owning_gate_without_unit_feature_unification(self):
        script = (ROOT / "scripts/component-runtime-v3.sh").read_text()
        line = next(line for line in script.splitlines()
                    if line.startswith("cargo test ") and line.endswith("wit_component::result_v3::tests::"))
        expected = [value.replace("$readonly_root", str(ROOT)) for value in shlex.split(line)[:-1]]
        self.assertEqual(TRACE["component_test_command"](), expected + ["--no-run", "--message-format=json"])
        self.assertNotIn("--all-features", expected)
        self.assertNotIn("--workspace", expected)

    def test_exact_failing_case_and_original_artifact_guards(self):
        prefix = "wit_component::result_v3::tests::"
        self.assertTrue(TRACE["CASE"].startswith(prefix))
        case = TRACE["CASE"][len(prefix):]
        source = (ROOT / "src/wit_component/result_v3/tests.rs").read_text()
        self.assertIn("fn " + case + "()", source)
        for name in ["select_executable", "elf_tls_layout", "FRAME_LAYOUT_CODE"]:
            self.assertIs(TRACE[name], TRACE["SUPPORT"][name])

    def test_non_hosted_execution_stops_before_a_process_starts(self):
        with patch.dict(os.environ, {"CI": "false"}), patch.object(TRACE["subprocess"], "check_output") as command:
            with self.assertRaisesRegex(ValueError, "owning hosted Linux CI runner"):
                TRACE["main"]()
            command.assert_not_called()

    def test_workflow_retains_owning_failure_and_bounds_trace(self):
        workflow = (ROOT / ".github/workflows/ci.yml").read_text()
        job = workflow.split("  component-runtime-v3:\n", 1)[1].split("\n  wasm-scalar-exports-browser-v1:", 1)[0]
        self.assertIn("id: component_gates", job)
        self.assertIn("run: scripts/component-runtime-v3.sh", job)
        self.assertIn("if: failure() && steps.component_gates.outcome == 'failure'", job)
        self.assertIn("timeout-minutes: 20", job)
        self.assertIn("python3 scripts/ci-component-stack-trace.py", job)
        self.assertNotIn("continue-on-error", job)
        self.assertNotIn("RUST_MIN_STACK", job)
        script = (ROOT / "scripts/ci-component-stack-trace.py").read_text()
        self.assertIn("timeout=180", script)
        self.assertIn('listing != [CASE] or ignored', script)
        self.assertIn('"--exact", "--nocapture", "--test-threads=1"', script)
        self.assertIn('current != expected', script)
