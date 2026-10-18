"""Prepare only; retain all174 subjects and the unchanged original binder."""
from pathlib import Path
import json,ast,hashlib,subprocess
R=Path('/Volumes/Semaprax/worktrees/focused146-structural-repairs-20261018');REL='docs/evidence/finite-overlay542-current174-20261018';N=REL+'/successor174/';D=R/REL;H=lambda b:hashlib.sha256(b).hexdigest()
def keep(p,b):q=R/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b)
def assignments(s,values):
 nodes={n.targets[0].id:n.value for n in ast.parse(s).body if isinstance(n,ast.Assign)and len(n.targets)==1 and isinstance(n.targets[0],ast.Name)};lines=s.splitlines(True);edits=[]
 for k,v in values.items():n=nodes[k];a=sum(map(len,lines[:n.lineno-1]))+n.col_offset;b=sum(map(len,lines[:n.end_lineno-1]))+n.end_col_offset;edits.append((a,b,repr(v)))
 for a,b,v in sorted(edits,reverse=True):s=s[:a]+v+s[b:]
 return s
bd=json.loads((R/'docs/evidence/main09013-source-namespaces-v1/binding-descriptor.json').read_bytes());manifest=(R/bd['manifest']).read_bytes();v=json.loads(manifest);assert len(v['physical'])==len({x['path']for x in v['physical']})==174
changed={}
for row in v['physical']:
 raw=(R/row['path']).read_bytes();pair=(len(raw),H(raw))
 if pair!=(row['current_len'],row['current_sha256']):changed[row['path']]=pair
old='docs/evidence/main-e177-final-source-binding-20261017/successor174/bind-next.py';before=(R/old).read_bytes();s=before.decode();names=('LEGACY','STRICT');values={n.targets[0].id:ast.literal_eval(n.value)for n in ast.parse(s).body if isinstance(n,ast.Assign)and len(n.targets)==1 and isinstance(n.targets[0],ast.Name)and n.targets[0].id in names};oldstrict=dict(values['STRICT'])
for p,row in values['LEGACY'].items():b=(R/p).read_bytes();row['actual']=(len(b),H(b))
helper='docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py'
# Current bounded external CONFIG and logical extension are acquired by the
# unchanged active guard's held readers, before preparing strict dependencies.
program=(R/helper).read_bytes();assert len(program)<=65536 and program==subprocess.check_output(['git','show','HEAD:'+helper],cwd=R)
namespace={'__file__':str(R/helper),'_AUTHENTICATED_PROGRAM':program};exec(compile(program,str(R/helper),'exec'),namespace)
cfg=namespace['CONFIG'];packet=namespace['packet']()
assert cfg['rows']==433 and cfg['logical_rows']==519 and len(packet['rows'])==542
needed={helper,'docs/evidence/merge-2e92-browser-20261018/guard-config.json',cfg['packet']['path'],cfg['extension']['path'],cfg['guard']['path'],cfg['guard']['archive'],cfg['public']['archive'],cfg['table']['path'],cfg['table']['prior']['archive']}|set(cfg['modules'])
# The active reader may use these two exact finite successor descriptors.
# Add only their explicitly named bounded sources and authenticated whole views.
excluded=[]
for key in ('source_successor','previous_successor'):
 pin=cfg[key];raw=(R/pin['path']).read_bytes();assert len(raw)==pin['length']<=262144 and H(raw)==pin['sha256'];needed.add(pin['path'])
 successor=json.loads(raw);assert len(successor['rows'])==len({r['source']for r in successor['rows']})
 for row in successor['rows']:
  if row['current']['length']>262144:
   excluded.append(dict(path=row['source'],reason='successor-Rust-only-current-exceeds-unchanged-Python-bound'));continue
  needed.add(row['source'])
  for image in [row['before'],row['current'],*row.get('views',[])]:
   if image['length']<=262144:needed.add(image['archive'])
   else:excluded.append(dict(path=image['archive'],reason='successor-view-exceeds-unchanged-Python-bound'))


for source in cfg['closed_prior_sources']:
 row=next((r for r in packet['rows']if r['source']==source),None)
 if row:
  # A legal invocation of this original Python reader cannot acquire a current
  # or archive image above its unchanged262144 bound. Rust custody is retained.
  if row['current']['length']>262144:
   excluded.append(dict(path=source,reason='Rust-only-current-exceeds-unchanged-Python-bound'));continue
  needed.update([source,row['current']['archive']])
  for pin in row['views']:
   if pin['length']<=262144:needed.add(pin['archive'])
   else:excluded.append(dict(path=pin['archive'],reason='view-exceeds-unchanged-Python-bound'))
 else:needed.add(source)

# The ONE fixed overlay page and its literal dependencies are part of the actual readset.
page_root='docs/evidence/finite-source-overlay-v2-20261018/'
needed.update(page_root+name for name in ('policy.json','page.json','overlay_page.py'))
page=json.loads((R/(page_root+'page.json')).read_bytes())
for row in [*page['replacements'],*page['additions']]:
 if row['current']['length']<=262144:needed.add(row['source'])
 else:excluded.append(dict(path=row['source'],reason='page-current-exceeds-original-Python-perread-bound'))
 for pin in [row['current'],*row['views']]:
  if pin['length']<=262144:needed.add(pin['archive'])
  else:excluded.append(dict(path=pin['archive'],reason='page-view-exceeds-original-Python-perread-bound'))

strict=values['STRICT']
for p in sorted(needed):strict.setdefault(p,None)
for p in strict:
 raw=(R/p).read_bytes();pair=(len(raw),H(raw))
 if len(raw)>262144:assert p in oldstrict and tuple(oldstrict[p])==pair and p=='docs/evidence/workspace21-legacy-source-repair-20261017/whole-source-inverses.json'and pair==(317943,'86f53656e6765ddc606675d1356d0fc91416c878b2d607373a4aed7bbac04b04'),p
 strict[p]=pair
s=assignments(s,dict(REL=N,BEFORE_LEN=len(manifest),BEFORE_SHA=H(manifest),CHANGED=changed,LEGACY=values['LEGACY'],STRICT=strict,STRICT_HELPER=helper));ast.parse(s)
for name,path in [('input-domains.before.json',bd['manifest']),('incoming090_semantic_receivers.before.rs.txt',bd['module']),('bind_current.before.py.txt',bd['binder']),('bind-next.before.py.txt',old)]:keep(N+name,(R/path).read_bytes())
keep(N+'bind-next.accepted-current.before.py.txt',(R/'docs/evidence/fmt-local441-current-source-ingress-20261018/successor174/bind-next.py').read_bytes());keep(N+'bind-next.py',s.encode());keep(REL+'/frozen174-preparation.json',(json.dumps(dict(status='PREPARED_NOT_EXECUTED',membership=174,changed=changed,strict_dependencies=len(strict),strict_cap=262144,unreadable_Rust_only_dependencies_not_added=excluded,logical_rows=542,only_inherited317943_exception_exact=True,binder_unchanged_sha256=H((R/bd['binder']).read_bytes()),whole_wrapper_before_sha256=H(before),wrapper_after_sha256=H(s.encode()),explicit_execution_expected_repo_and_HEAD_required=True),indent=2)+'\n').encode());print('PREPARED_NOT_EXECUTED',len(changed),len(strict))
