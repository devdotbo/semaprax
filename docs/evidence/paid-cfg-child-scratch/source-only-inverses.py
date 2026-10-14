#!/usr/bin/env python3
"""Whole original source reconstruction only; no Rust or Cargo execution."""
from pathlib import Path
import json,hashlib
root=Path(__file__).resolve().parents[3];folder=Path(__file__).parent
sha=lambda b:hashlib.sha256(b).hexdigest()
for row in json.loads((folder/'exact-source-inverses.json').read_text()):
    current=(root/row['source']).read_text()
    for edit in row['edits']:
        assert current.count(edit['after'])==1
        current=current.replace(edit['after'],edit['before'],1)
    archive=(root/row['archive']).read_bytes()
    assert current.encode()==archive
    assert sha(archive)==row['before_sha256']
assert (root/'src/loan_plan/paid_cfg/original.rs').read_bytes()==(folder/'src__loan_plan__paid_cfg.rs.before.txt').read_bytes()
packet=json.loads((folder/'source-binding.json').read_text())
for path,row in packet['current_sources'].items():assert sha((root/path).read_bytes())==row['sha256']
for path,digest in packet['unchanged_complete_originals_and_common_helpers'].items():assert sha((root/path).read_bytes())==digest
print('Source-only: five whole source inverses exact, complete original373-line paid CFG independent, legacy whole route/helper bodies unchanged; actual owning gates pending.')
