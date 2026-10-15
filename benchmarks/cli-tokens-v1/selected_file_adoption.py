#!/usr/bin/env python3
"""Owning reference gate only: original CLI49; no model, campaign or benchmark launch."""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import live_campaign
import qualification


def qualify(candidate: Path, output: Path, env: dict[str, str]) -> dict:
    metadata = qualification.metadata()
    settings = {"round": 6, "qualification": metadata, "seed_files_sha256": {
        "benchmarks/cli-tokens-v1/SPEC.md": hashlib.sha256((live_campaign.BENCHMARK / "SPEC.md").read_bytes()).hexdigest()}}
    result = live_campaign.check_program_for_campaign(candidate, 120, env, settings, "semaprax", output)
    if not result.get("accepted") or len(result.get("checks", [])) != 33 or len(result.get("boundary_checks", [])) != 16:
        raise ValueError("selected-file LogLens must pass the unchanged historical33 and all16 SPEC boundary checks")
    if result["execution"]["mode"] != "native_executable_via_wrapper":
        raise ValueError("selected-file LogLens owning gate requires actual native executable evidence")
    return result


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--candidate", required=True, type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()
    result = qualify(args.candidate.resolve(strict=True), args.output.absolute(), dict(os.environ))
    print(json.dumps({"accepted": result["accepted"], "historical_checks": len(result["checks"]),
                      "boundary_checks": len(result["boundary_checks"]), "benchmark_executed": False}))
    return 0

if __name__ == "__main__":
    raise SystemExit(main())
