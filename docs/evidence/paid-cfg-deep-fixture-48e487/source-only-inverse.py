from pathlib import Path
import hashlib
base=Path(__file__).resolve().parent
root=base.parents[2]
current=(root/'src/loan_plan/paid_cfg/tests.rs').read_bytes()
added=(base/'current-fixture.txt').read_bytes()
original=(base/'original-fixture.txt').read_bytes()
assert current.count(added)==1
restored=current.replace(added,original,1)
assert restored==(base/'tests.rs.before.txt').read_bytes()
print('PASS complete original test source restored; no body/measurement/assertion excluded')
