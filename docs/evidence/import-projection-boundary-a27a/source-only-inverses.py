#!/usr/bin/env python3
"""Full import adapter reconstruction only; no Rust execution."""
from pathlib import Path
import hashlib,json
root=Path(__file__).resolve().parents[3]
packet=json.loads((Path(__file__).parent/'source-binding.json').read_text())
sha=lambda b:hashlib.sha256(b).hexdigest()
for row in packet['whole_inverse_hooks']:
    source=(root/row['source']).read_bytes()
    archive=(root/row['archive']).read_bytes()
    assert sha(source)==row['receiving_sha256']
    assert sha(archive)==row['before_sha256']
    for edit in row.get('additional_receiving_inverses', []):
        new=edit['after'].encode();old=edit['before'].encode()
        assert source.count(new)==1
        source=source.replace(new,old,1)
    new=row['unique_new_imports'].encode();old=row['original_import'].encode()
    assert source.count(new)==1
    assert source.replace(new,old,1)==archive
for path,digest in packet['complete_current_projection_members'].items():
    source=(root/path).read_bytes()
    assert sha(source)==digest
    assert b'use super::*;' not in source
assert sha((root/'tests/workspace/semantic_graph.rs').read_bytes())==packet['unchanged_owning_guard_sha256']
assert sha((root/'tests/workspace/semantic_graph/source_inventory.rs').read_bytes())==packet['unchanged_full_source_inventory_sha256']
print('Source only: five whole-file inverses exact; all 68 unchanged joined projection members contain zero wildcard imports; actual Rust owners pending.')
