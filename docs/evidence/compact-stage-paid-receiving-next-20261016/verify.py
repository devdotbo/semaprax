#!/usr/bin/env python3
import hashlib,json,pathlib,subprocess,re
root=pathlib.Path(__file__).resolve().parents[3];packet=pathlib.Path(__file__).resolve().parent
m=json.loads((packet/'source-inverses.json').read_text());sha=lambda b:hashlib.sha256(b).hexdigest()
for row in m['rows']:
    current=(root/row['path']).read_bytes();before=(packet/'before'/row['path']).read_bytes()
    assert sha(current)==row['after_sha256'] and sha(before)==row['before_sha256']
    assert before==subprocess.check_output(['git','show',m['base']+':'+row['path']],cwd=root)
    restored=current.decode()
    for e in reversed(row['edits']):
        assert restored.count(e['after'])==1
        restored=restored.replace(e['after'],e['before'],1)
    assert restored.encode()==before
    if row['path'].endswith('/tests.rs'):
        old=before.decode();new=current.decode()
        for match in re.finditer(r'\bassert(?:_eq|_ne)?!\(',old):
            start=match.start();end=match.end();level=1
            while level:
                if old[end]=='(':level+=1
                elif old[end]==')':level-=1
                end+=1
            assert old[start:end] in new
s=(root/'src/cleanup_plan/replay/compact_stage.rs').read_text()
assert 'use super::*;' not in s
assert '#[cfg(test)]\nimpl Residual' not in s
assert 'source: StagedCopyResultSource' in s
oracle=(root/'src/cleanup_plan/replay/factored/oracle_order.rs').read_text()
assert 'staged(a.canonical(), b.canonical(), function, budget)' not in oracle
assert 'canonical_staged!(a, b, function, budget)' in oracle
print('PASS six complete Main source inverses, all original test assertions, explicit imports and lexical residual comparison')
