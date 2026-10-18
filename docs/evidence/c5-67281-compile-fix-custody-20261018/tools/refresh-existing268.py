from pathlib import Path
import argparse,json,subprocess,hashlib,copy,os
p=argparse.ArgumentParser();p.add_argument('--head',required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--write',action='store_true');a=p.parse_args();R=Path('/Volumes/Semaprax/worktrees/next-large-combined-20261017');BASE='67281d0166b024a0c13e36eb16d271065c1da8f8';G=lambda *v:subprocess.check_output(['git',*v],cwd=R);H=lambda b:hashlib.sha256(b).hexdigest();E=lambda x:(json.dumps(x,indent=2)+'\n').encode();assert G('rev-parse','HEAD').decode().strip()==a.head and not G('status','--porcelain').strip();assert not a.out.exists();a.out.mkdir();F='docs/evidence/a3-e47-final-custody-20261016/';B='docs/evidence/source-boundary-domains-20261016/';REL='docs/evidence/c5-67281-compile-fix-custody-20261018';rawchanges={'src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase.rs','src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase_tests.rs'};allowed=rawchanges|{'src/source_reflows_merge/main_e177_final_inputs.rs'}
snap={};old={};models={};edge_changes=[]
def held(path):
 b=(R/path).read_bytes();assert b==G('show',a.head+':'+path)
 if path in snap:assert b==snap[path]
 else:snap[path]=b
 return b
for name in ['hir','projection']:
 path=F+name+'.json';old[path]=held(path);assert old[path]==G('show',BASE+':'+path);m=json.loads(old[path]);before=copy.deepcopy(m)
 for path,pin in m['sources'].items():
  raw=held(path)
  if (len(raw),H(raw))!=(pin['length'],pin['sha256']):assert path in allowed;pin.update(length=len(raw),sha256=H(raw))
 for path,c in m['classes'].items():
  for chain in c['chains']:
   for e in chain['edges']:
    b=held(e['parent']);text=e['text'].encode();n=e['start']
    if b[n:n+len(text)]!=text:
     assert e['parent']=='src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase.rs';prior=G('show',BASE+':'+e['parent']);assert prior[n:n+len(text)]==text and prior.count(text)==b.count(text)==1
     e['start']=b.index(text);edge_changes.append(dict(domain=name,source=path,parent=e['parent'],child=e['child'],before=n,after=e['start'],text=e['text']))
    assert b[e['start']:e['start']+len(text)]==text
 # The only class difference is the actual two module declarations' offsets.
 restored=copy.deepcopy(m)
 for path,c in restored['classes'].items():
  for ci,chain in enumerate(c['chains']):
   for ei,e in enumerate(chain['edges']):e['start']=before['classes'][path]['chains'][ci]['edges'][ei]['start']
 restored['sources']=before['sources'];assert restored==before;models[name]=m
assert {(x['parent'],x['child'])for x in edge_changes}=={('src/cleanup_plan/replay/expression_facts/receiving/retained/ordinary_cross_phase.rs','src/cleanup_plan/replay/expression_facts/receiving/retained/'+n)for n in ['ordinary_cross_phase_before.rs','ordinary_cross_phase_tests.rs']}
own=F+'new-owners.json';old[own]=held(own);assert old[own]==G('show',BASE+':'+own);owners=json.loads(old[own]);assert len(owners)==268
for x in owners:
 b=held(x['source']);prior=(x['sha256'],x['lines']);x.update(sha256=H(b),lines=len(b.splitlines()))
 if prior!=(x['sha256'],x['lines']):assert x['source']in allowed
for name,m in models.items():(a.out/(name+'.json')).write_bytes((json.dumps(m,separators=(',',':'))+'\n').encode())
toolpath=B+'boundary_tools.py';tool=held(toolpath);assert tool==G('show',BASE+':'+toolpath);ns={'__file__':str(R/toolpath)};exec(compile(tool,toolpath,'exec'),ns)
gen=held(F+'generate-authority.py');assert H(gen)=='0c4b31857cdb8a4f645d61c6f0d2453c33c6adddb8c6f26d9c37b56620841590';prefix=gen.decode().split("for name,chunks in [('hir',hir_chunks),('projection',projection_chunks)]:",1)[0]
manifest,proof=ns['generated_scope'](R,a.out,regenerated=(models,H(prefix.encode())));proof['negative_controls']=ns['negative_checks'](R,a.out,manifest);proof['status']='SOURCE_ONLY_FINITE_EXISTING268_HASH_AND_TWO_EDGE_OFFSET_SUCCESSOR';proof['registration_update']='No parser reenumeration; complete prior268 classes/chunks retained with two exact declaration offsets relocated.'
outputs={F+n+'.json':(a.out/(n+'.json')).read_bytes()for n in models};outputs[own]=E(owners)
for path,val in [(B+'current-domains.json',manifest),(B+'boundary-source-proof.json',proof)]:old[path]=held(path);assert old[path]==G('show',BASE+':'+path);outputs[path]=E(val)
historic=F+'authority-source-proof.json';assert held(historic)==G('show',BASE+':'+historic)
records=[]
for path,b in old.items():
 archive=REL+'/models-before/'+path.replace('/','__')+'.txt';records.append(dict(path=path,archive=archive,before_sha256=H(b),after_sha256=H(outputs[path])))
receipt=dict(status='PASS_SOURCE_ONLY_EXISTING268_REFRESH',head=a.head,baseline=BASE,owners=268,edge_changes=edge_changes,negative_controls=proof['negative_controls'],records=records,written=a.write,Cargo=False);(a.out/'receipt.json').write_bytes(E(receipt))
if a.write:
 assert G('rev-parse','HEAD').decode().strip()==a.head and not G('status','--porcelain').strip()
 for path,b in snap.items():assert (R/path).read_bytes()==b
 for rec in records:
  q=R/rec['archive'];assert not q.exists();q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(old[rec['path']])
 for path,b in outputs.items():(R/path).write_bytes(b)
 (R/REL/'model-refresh-result.json').write_bytes(E(receipt))
print(json.dumps(dict(status=receipt['status'],owners=268,negative_controls=receipt['negative_controls'],written=a.write)))
