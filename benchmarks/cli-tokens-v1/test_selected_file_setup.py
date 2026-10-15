import hashlib
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import subprocess
import selected_file_setup as setup


class SelectedFileSetupTests(unittest.TestCase):
    def test_default_setup_and_typescript_prompt_are_unchanged(self):
        with patch.object(setup, "capability", side_effect=AssertionError("default must not probe")):
            self.assertEqual(setup.prompt_note({}, "semaprax", Path("absent")), "")
            self.assertEqual(setup.prompt_note({}, "typescript", Path("absent")), "")
        with patch.object(setup, "capability", return_value={"exact": True}):
            self.assertEqual(setup.prompt_note({"selected_file_setup": {"exact": True}}, "typescript", Path("compiler")), "")
            self.assertEqual(setup.prompt_note({"selected_file_setup": {"exact": True}}, "semaprax", Path("compiler")), setup.NOTE)

    def test_exact_compiler_capability_and_stale_setup_fail_closed(self):
        help_text = f"{setup.PROFILE} {setup.INPUT} native64 Unix 256 1048576".encode()
        with tempfile.TemporaryDirectory() as temp:
            binary = Path(temp) / "compiler"
            binary.write_bytes(b"selected source compiler")
            valid = subprocess.CompletedProcess([], 0, help_text, b"")
            with patch.object(setup.subprocess, "run", return_value=valid):
                selected = setup.capability(binary)
                self.assertEqual(selected["compiler_binary_sha256"], hashlib.sha256(binary.read_bytes()).hexdigest())
                self.assertEqual(selected["qualification_checks"], 49)
                self.assertEqual(setup.prompt_note({"selected_file_setup": selected}, "semaprax", binary), setup.NOTE)
                stale = {**selected, "input": "argv-utf8+file-text.v1"}
                with self.assertRaises(ValueError):
                    setup.prompt_note({"selected_file_setup": stale}, "semaprax", binary)
            for bad in [subprocess.CompletedProcess([], 1, help_text, b""),
                        subprocess.CompletedProcess([], 0, b"old profile only", b"")]:
                with patch.object(setup.subprocess, "run", return_value=bad):
                    with self.assertRaises(ValueError):
                        setup.capability(binary)


if __name__ == "__main__":
    unittest.main()
