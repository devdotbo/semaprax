"""Read-only exact source reauthentication after all owning source binders."""
from pathlib import Path
import argparse,os,stat,re,json,hashlib,ast,subprocess
P=argparse.ArgumentParser();P.add_argument('--repo',type=Path,required=True);P.add_argument('--out',type=Path,required=True);a=P.parse_args();R=a.repo.resolve();assert R.is_relative_to('/Volumes/Semaprax/worktrees');assert not a.out.exists()
H=lambda b:hashlib.sha256(b).hexdigest();CAP=9753094
SNAPS={}
def held(path,length,digest,cap=CAP):
 p=R/path;assert 0<=length<=cap;key=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns);before=p.lstat();assert stat.S_ISREG(before.st_mode)and before.st_size==length
 fd=os.open(p,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 try:
  assert key(os.fstat(fd))==key(before)
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(length+1)
  assert len(b)==length and key(os.fstat(fd))==key(before)
 finally:os.close(fd)
 assert key(p.lstat())==key(before)and H(b)==digest
 if path in SNAPS:assert SNAPS[path]==b
 else:SNAPS[path]=b
 return b
ST='docs/evidence/main-e177-final-source-binding-20261017';helper=(R/'src/source_reflows_merge/main_e177_final_inputs.rs').read_bytes();text=helper.decode();length=int(re.search(r'assert_eq!\(data.len\(\), (\d+)\);',text)[1]);digest=re.search(r'assert_eq!\(digest\(data.as_bytes\(\)\), "([0-9a-f]{64})"\);',text)[1]
raw=held(ST+'/current-inputs.json',length,digest,262144);packet=json.loads(raw);assert packet['source_cap']==CAP and len(packet['rows'])==98
compiled=text.split('fn compiled(',1)[1].split('fn read_view(',1)[0];tables=text.split('fn table_compiled(',1)[1].split('fn compiled(',1)[0]
arm=re.compile(r'"([^"\n]+)" => Some\(include_str!\("\.\./\.\./([^"\n]+)"\)\)');channel=dict(arm.findall(compiled));tablechannel=dict(arm.findall(tables));assert len(channel)==len(arm.findall(compiled))
views=0
for row in packet['rows']:
 p=row['current'];actual=held(row['source'],p['length'],p['sha256']);assert actual==held(p['archive'],p['length'],p['sha256'])
 if row['compiled']:assert channel[row['source']]==row['source']
 for p in row['views']:held(p['archive'],p['length'],p['sha256']);views+=1
for p in packet['complete_old_table_anchors']:
 actual=held(p['path'],p['length'],p['sha256'],262144);assert actual==held(p['archive'],p['length'],p['sha256'],262144);assert tablechannel[p['path']]==p['path']
guardraw=(R/(ST+'/guard_payload.py')).read_bytes();cfg=next(ast.literal_eval(n.value)for n in ast.parse(guardraw).body if isinstance(n,ast.Assign)and isinstance(n.targets[0],ast.Name)and n.targets[0].id=='CONFIG');assert cfg['rows']==98 and len(cfg['modules'])==57 and len(cfg['closed_prior_sources'])==69
for source,p in cfg['modules'].items():held(source,p['length'],p['sha256'],262144)
for source,p in cfg['carriers'].items():
 body=held(source,p['length'],p['sha256'],262144).decode()
 for module in cfg['modules']:assert body.count('include_str!("'+p['base']+module+'")')==1,(source,module)
 if 'ledger_capacity'in source:assert len(body.splitlines())<=1500
p=cfg['table'];body=held(p['path'],p['length'],p['sha256'],262144);prior=p['prior'];expected=held(prior['archive'],prior['length'],prior['sha256'],262144).decode().splitlines()
for x in p['insertions']:assert x['path']not in expected;expected.insert(x['at'],x['path'])
assert body==('\n'.join(expected)+'\n').encode()and len(expected)==len(set(expected))==462
owners=json.loads((R/'docs/evidence/a3-e47-final-custody-20261016/new-owners.json').read_bytes());assert len(owners)==241
for p in owners:
 b=(R/p['source']).read_bytes();assert H(b)==p['sha256']and len(b.decode().splitlines())==p['lines']
# Re-read every authority/source byte after the entire scan, preserving its original tuple.
for path,b in list(SNAPS.items()):assert held(path,len(b),H(b))==b
assert (R/'src/source_reflows_merge/main_e177_final_inputs.rs').read_bytes()==helper and(R/(ST+'/guard_payload.py')).read_bytes()==guardraw
j=dict(status='POSTBIND_SOURCE_ONLY_FULL_REAUTH_PASS',input_HEAD=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip(),current_rows=98,retained_source_views=views,complete_old_table_anchors=len(packet['complete_old_table_anchors']),literal_registered_compiled_channels=len(channel),actual_new_compiled_binary_claim=False,actual_required_modules=57,ordered_rows=462,role_owners=241,Python_closed_channels=69,distinct_held_paths=len(SNAPS),packet_sha256=H(raw),guard_sha256=H(guardraw),source_cap_unchanged=CAP,Python_small_cap_unchanged=262144,Cargo=False,Rust=False,source_mutations=False)
a.out.write_text(json.dumps(j,indent=2)+'\n');print(json.dumps(j))
