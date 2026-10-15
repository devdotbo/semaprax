"""Apply only the already strict-validated historical1a0b literal plan privately.
This does not rerun the moved ROOT's producer or claim current production output.
"""
import difflib, hashlib, json, shutil, subprocess
from pathlib import Path
R=Path('/Volumes/Semaprax/worktrees/sharing-actual-pins-1a0b-20261015');ROOT=Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang');O=Path('/Volumes/Semaprax/provenance/sharing-pin-plan-1a0b-20261015');E=R/'docs/evidence/sharing-actual-pins-1a0b-20261015'
def sha(data):return hashlib.sha256(data).hexdigest()
def git(repo,*args):return subprocess.check_output(['git',*args],cwd=repo,text=True).strip()
head='1a0b9d9bc5ce2b36133d61ea4636dc9614f2f41e';assert git(R,'rev-parse','HEAD')==head and not git(R,'status','--porcelain','--untracked-files=all');assert not E.exists()
plan=json.loads((O/'readonly-plan.json').read_bytes());assert plan['capture_head']==head and len(plan['edits'])==11
assert sha((O/'owning-build-result.json').read_bytes())=='8b20fc0eae9bf233bcd2bbf051d0c2ab33b45714f5fa9298c1730b86b3e012a0'
assert sha((O/'prelude-pin-producers-result.json').read_bytes())=='363305bcd34fb7d7f9e05c96f5569842ee6f7c239f444b8a7239eac6e17e9102'
before={};after={}
for path,digest in plan['source_sha256'].items():
 data=(R/path).read_bytes();assert 'sha256:'+sha(data)==digest;assert (ROOT/path).read_bytes()==data;assert b'\r' not in data;before[path]=data;after[path]=data
for edit in plan['edits']:
 path=edit['path'];old=edit['before'].encode();new=edit['after'].encode();assert after[path].count(old)==1;after[path]=after[path].replace(old,new,1)
for path,data in after.items():assert 'sha256:'+sha(data)==plan['after_sha256'][path]
# Compose the existing capture/source guard inverse, preserving every old edit,
# original archive and historical hash. Its consumer replays edits in reverse.
meta='docs/evidence/prelude-forecast-286/pin-capture/exact-source-inverses.json';before[meta]=(R/meta).read_bytes();rows=json.loads(before[meta]);row=next(r for r in rows if r['path']=='tests/workspace/semantic_graph.rs');prior=list(row['edits']);added=[{'before':e['before'],'after':e['after']} for e in plan['edits'] if e['path']==row['path']];assert len(added)==10;row['edits'].extend(added);assert row['edits'][:len(prior)]==prior
after[meta]=(json.dumps(rows,indent=2)+'\n').encode()
restored=after['tests/workspace/semantic_graph.rs'].decode()
for edit in row['edits'][::-1]:
 assert restored.count(edit['after'])==1,edit['after'][:80];restored=restored.replace(edit['after'],edit['before'],1)
original=(R/'docs/evidence/prelude-forecast-286/pin-capture'/row['archive']).read_bytes();assert restored.encode()==original and sha(original)==row['sha256']
root_head=git(ROOT,'rev-parse','HEAD');assert root_head!=head
# All private/root target old bytes preflight before ANY private source write.
for path,data in before.items():assert (R/path).read_bytes()==data and (ROOT/path).read_bytes()==data
E.mkdir(parents=True);shutil.copytree(O/'actual-current-capture',E/'actual-current-capture');shutil.copytree(O/'actual-previous-pinned-capture',E/'actual-previous-pinned-capture')
for name in ['adaptation.json','original-updater.py','update-prelude-forecast-pins-1a0b-only.py','readonly-command.json','readonly-plan.json','readonly-stderr.txt','whole-wire-audit.json','whole-wire-exact-inverses.json','owning-build-result.json','prelude-pin-producers-result.json','expected-counts.json']:
 shutil.copyfile(O/name,E/name)
shutil.copyfile(Path(__file__),E/'apply_validated_plan_private.py')
inverses=[]
for path,data in before.items():
 archive=E/'original'/path;archive.parent.mkdir(parents=True,exist_ok=True);archive.write_bytes(data)
 inv=E/(path.replace('/','__')+'.inverse.patch');inv.write_text(''.join(difflib.unified_diff(after[path].decode().splitlines(True),data.decode().splitlines(True),fromfile='a/'+path,tofile='b/'+path)))
 inverses.append({'path':path,'current_sha256':sha(after[path]),'original_sha256':sha(data),'archive':str(archive.relative_to(R)),'inverse_patch':str(inv.relative_to(R)),'inverse_patch_sha256':sha(inv.read_bytes())})
record={'status':'PRIVATE_PIN_ONLY_APPLICATION_OF_ALREADY_VALIDATED_HISTORICAL_PLAN','capture_source_head':head,'private_application_basis':git(R,'rev-parse','HEAD'),'observed_later_root_head':root_head,'root_pin_and_inverse_owner_bytes_identical_to_capture_basis':True,'strict_readonly_plan_preceded_root_advance':True,'strict_readonly_plan_sha256':sha((O/'readonly-plan.json').read_bytes()),'trusted_build_receipt_sha256':sha((O/'owning-build-result.json').read_bytes()),'trusted_producer_receipt_sha256':sha((O/'prelude-pin-producers-result.json').read_bytes()),'edit_count':11,'root_mutated':False,'producer_rerun':False,'qualification':'Pin owners must compile and pass all remaining original assertions unchanged after integration. Current later ROOT output has not been captured or claimed.','source_inverses':inverses,'complete_original_pin_capture_inverse_reconstructs_archive':True}
(E/'application-record.json').write_text(json.dumps(record,indent=2)+'\n')
for path,data in before.items():assert (R/path).read_bytes()==data
for path,data in after.items():(R/path).write_bytes(data)
# Verify complete source-byte inverses on disposable copies only.
T=E/'inverse-check';T.mkdir()
for row in inverses:
 q=T/row['path'];q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(after[row['path']])
subprocess.run(['git','apply','--unsafe-paths','--directory='+str(T),*[str(R/r['inverse_patch']) for r in inverses]],cwd=R,check=True)
for row in inverses:assert sha((T/row['path']).read_bytes())==row['original_sha256']
shutil.rmtree(T)
print(json.dumps({'private_repository':str(R),'later_root_head':root_head,'literal_edits':11,'whole_owner_inverse_rows':len(inverses),'full_original_pin_capture_inverse':True,'root_mutated':False}))
