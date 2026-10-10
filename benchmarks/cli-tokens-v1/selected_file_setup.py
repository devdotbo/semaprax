"""Opt-in future LogLens setup. Frozen task, TypeScript prompt and CLI49 stay exact."""
from __future__ import annotations
import hashlib
import subprocess
from pathlib import Path

PROFILE = "source-command.selected-file.resource-output.v1"
INPUT = "argv-utf8-256+selected-file.follow.v1"
SCHEMA = "semaprax.loglens-selected-file-setup.v1"
NOTE = f"""Explicit SEMAPRAX native input setup for this future trial:
The selected compiler advertises the additive Project profile {PROFILE}, input {INPUT},
semantic contract semaprax.project.v34, native64 Unix only. Declare exact
capabilities fs.read, process.args.read, process.stderr.write, process.stdout.write.
Use the ordinary checked native Project build. The run wrapper may locate its
built executable and must forward all original application arguments unchanged:
with zero arguments invoke it directly; otherwise invoke it as
./loglens --spx-selected-file-v1 0 -- "$@".
The fixed host prefix selects application argv index 0; checked source sees
<file> and every original option in its original order. Checked source must
validate complete usage BEFORE file_read_text. The host opens only the exact
selected key lazily, follows selected symlinks and holds one regular descriptor;
other source Strings confer no filesystem authority. Do not implement dirname,
cd or readlink traversal for the input, or normalize options in the shell.
Bounds: 256 raw argv and 65536 total argument bytes; 4096 selected path bytes;
65536 file bytes; inherited 1 MiB String and combined staged output envelopes.
Preserve all frozen LogLens rules, decimal exactness and exit statuses; these
physical invocation bounds add no numeric byte-count ceiling. Embedded help:
help language author:selected-file-resource. The TypeScript task is unchanged.
This setup supplies capability guidance, not an implementation or acceptance oracle.
"""


def capability(binary: Path) -> dict:
    result = subprocess.run([str(binary), "help", "language", "author:selected-file-resource"],
                            capture_output=True, timeout=30, check=False)
    required = [PROFILE, INPUT, "native64 Unix", "256", "1048576"]
    if result.returncode != 0 or any(token.encode() not in result.stdout for token in required):
        raise ValueError("selected-file resource setup requires the exact supported compiler help route")
    return {"schema": SCHEMA, "profile": PROFILE, "input": INPUT,
            "compiler_binary_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(),
            "help_sha256": hashlib.sha256(result.stdout).hexdigest(),
            "prompt_note_sha256": hashlib.sha256(NOTE.encode()).hexdigest(),
            "helper_sha256": hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
            "task_spec_unchanged": True, "qualification_checks": 49,
            "typescript_prompt_unchanged": True}


def prompt_note(settings: dict, arm: str, binary: Path) -> str:
    selected = settings.get("selected_file_setup")
    if selected is None:
        return ""
    if selected != capability(binary):
        raise ValueError("selected-file resource capability/setup drifted after the immutable plan")
    if arm not in ("semaprax", "typescript"):
        raise ValueError("unknown matched LogLens arm")
    return NOTE if arm == "semaprax" else ""
