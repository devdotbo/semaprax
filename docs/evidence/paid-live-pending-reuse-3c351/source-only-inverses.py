from pathlib import Path
import json,hashlib
base=Path(__file__).resolve().parent;root=base.parents[2]
for row in json.loads((base/'whole-source-inverses.json').read_text()):
 current=(root/row['source']).read_text()
 for hook in row['hooks']:
  assert current.count(hook['after'])==hook['count']
  current=current.replace(hook['after'],hook['before'])
 original=(root/row['archive']).read_bytes()
 assert current.encode()==original
 assert hashlib.sha256(original).hexdigest()==row['before_sha256']
original=(root/'src/loan_plan/paid_live/original.rs').read_text()
assert original.count('#[path = "tests.rs"]\n')==1
assert original.replace('#[path = "tests.rs"]\n','',1)==(base/'paid_live.rs.before.txt').read_text()
original=(root/'src/loan_plan/paid_plan/pre_live_reuse.rs').read_text()
alias='use super::super::paid_live::before_plan_live as paid_live;\n'
assert original.count(alias)==1
assert original.replace(alias,'',1)==(base/'paid_plan.rs.before.txt').read_text()
original=(root/'src/loan_plan/paid_live/funded_before.rs').read_text()
alias='use super::before_plan as paid_plan;\n';assert original.count(alias)==1;original=original.replace(alias,'',1)
for name in ['tests','directories_tests','work_boundary']:
 path=f'#[path = "../funded_flat/{name}.rs"]\n';assert original.count(path)==1;original=original.replace(path,'',1)
assert original==(base/'funded_flat.rs.before.txt').read_text()
binding=json.loads((base/'source-binding.json').read_text())
for path,digest in binding['complete_unchanged_receivers_and_common_helpers'].items():
 assert hashlib.sha256((root/path).read_bytes()).hexdigest()==digest
print('PASS3 complete source inverses and full independent live/plan/funded receivers; all bound old helpers untouched')
