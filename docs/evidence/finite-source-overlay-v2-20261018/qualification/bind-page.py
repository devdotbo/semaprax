from pathlib import Path
import json,hashlib,subprocess,copy,re,os,argparse
ap=argparse.ArgumentParser();ap.add_argument('--write',action='store_true');a=ap.parse_args()
R=Path('/Volumes/Semaprax/worktrees/focused146-structural-repairs-20261018');D=Path(__file__).parent;E='docs/evidence/finite-source-overlay-v2-20261018';G=lambda *x:subprocess.check_output(['git',*x],cwd=R);H=lambda b:hashlib.sha256(b).hexdigest();canon=lambda v:(json.dumps(v,sort_keys=True,ensure_ascii=False,separators=(',',':'))+'\n').encode()
head=G('rev-parse','HEAD').decode().strip();assert not G('status','--porcelain').strip()
oldraw=Path('/Volumes/Semaprax/provenance/focused146-overlay-binding-scope-20261018/whole519-b5a7-canonical.json').read_bytes();assert H(oldraw)=='03e3a45b125ba9e231ad01100e0d91a1bdef2d0602e2d1f87e7e4bd5994a23ab';before=json.loads(oldraw);assert canon(before)==oldraw
held={};writes={};archives={}
def live(p):
 if p not in held:
  q=R/p;assert q.is_file()and not q.is_symlink();b=q.read_bytes();assert b==G('show',head+':'+p);held[p]=b
 return held[p]
def archive(b):
 p='e/'+H(b)
 if (R/p).exists():assert (R/p).is_file()and not(R/p).is_symlink()and(R/p).read_bytes()==b
 archives[p]=b;return dict(length=len(b),sha256=H(b),archive=p)
by={r['source']:r for r in before['rows']};assert len(by)==519
control={'src/source_reflows_merge/main_e177_final_inputs.rs','src/source_reflows_merge/main_e177_overlay_v2.rs',E+'/overlay_page.py',E+'/policy.json',E+'/page.json','docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py','docs/evidence/merge-2e92-browser-20261018/guard-config.json'}
classification=json.loads(Path('/Volumes/Semaprax/provenance/ordinary-replay-receiving-892-final-union-20261018/ORIGINAL-PARSER-CLASSIFICATION.json').read_text());classes={x['path']:x for x in classification};newbusiness=[x['path']for x in classification if x['path'].endswith('.rs')and x['path']not in by];errors='src/cleanup_plan/replay/expression_facts/errors.rs';newrust=newbusiness+[errors];assert len(newrust)==len(set(newrust))==22
for p in newbusiness:assert classes[p]['discovery']and all(x['kind']=='lib'for x in classes[p]['discovery'])
# Full authentic source generations, never normalized. Existing named views stay exact.
refs=['b5a7f2ba01b2a17a943603e57e9fa8a3b0988a56','c86d466a05629f7002fad118bcf4b650b1fdbca6','1a58947f0c12e745f328db2f7fa1a6639f98edf9','3692cd3c0916518fcf27b3286d522ace89d04f1b','f3db66f4240ca6b3cc986d6080d428438b606141']
def histories(row):
 p=row['source'];pins={(v['length'],v['sha256'])for v in [row['current'],*row['views']]}
 for ref in refs:
  r=subprocess.run(['git','show',ref+':'+p],cwd=R,stdout=subprocess.PIPE,stderr=subprocess.DEVNULL)
  if r.returncode:continue
  b=r.stdout;k=(len(b),H(b))
  if k not in pins:row['views'].append(dict(name='whole-'+ref[:12]+'-source',**archive(b)));pins.add(k)
rows=copy.deepcopy(before['rows']);positions={r['source']:i for i,r in enumerate(rows)};replaced=[]
for row in rows:
 p=row['source']
 if p in control:continue
 b=live(p);pin=row['current']
 if(len(b),H(b))!=(pin['length'],pin['sha256']):
  row['views'].append(dict(name='whole-pre-overlay-v2-current',**pin));row['current']=archive(b);histories(row);replaced.append(p)
added=[]
for p in newrust+['docs/ORDINARY-REPLAY-RECEIVING-V1.md']:
 assert p not in by and p not in control
 row=dict(source=p,compiled=p.endswith('.rs'),current=archive(live(p)),views=[]);histories(row);rows.append(row);positions[p]=len(rows)-1;added.append(p)
newby={r['source']:r for r in rows};roots=['crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs','tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs'];edges=[]
for p in newrust:assert newby[p]['compiled']is True and not newby[p].get('fragments')
for root in roots:
 row=newby[root];assert root in replaced;row.setdefault('fragments',[])
 for p in newrust:row['fragments'].append(copy.deepcopy(newby[p]));edges.append(dict(parent=root,child=p))
# Refresh every original canonical fragment object to its exact current standalone row.
for row in rows:
 if 'fragments'in row:
  row['fragments']=[copy.deepcopy(newby[x['source']])for x in row['fragments']]
  if row['source']not in replaced:
   replaced.append(row['source'])
replaced=[r['source']for r in before['rows']if r['source']in set(replaced)]
basepolicy=json.loads(live(E+'/policy.json'));pred=basepolicy['predecessor'];page=dict(schema='semaprax.finite-source-overlay-page.v2',index=1,total_pages=1,predecessor=pred,replacements=[newby[p]for p in replaced],additions=[newby[p]for p in added]);pagebytes=canon(page);assert len(pagebytes)<=262144
policy=dict(schema='semaprax.finite-source-overlay-policy.v2',predecessor=pred,page=dict(path=E+'/page.json',length=len(pagebytes),sha256=H(pagebytes)),replacements=replaced,additions=[dict(source=p,compiled=newby[p]['compiled'])for p in added],new_fragment_edges=edges);policybytes=canon(policy);assert len(policybytes)<=262144
ns={};exec(compile(live(E+'/overlay_page.py'),E+'/overlay_page.py','exec'),ns);logical=ns['compose_values'](before,policy,page);assert len(logical['rows'])==542
writes[E+'/page.json']=pagebytes;writes[E+'/policy.json']=policybytes
rust='src/source_reflows_merge/main_e177_overlay_v2.rs';s=live(rust).decode()
s,n=re.subn(r'const REPLACEMENTS: &\[&str\] = &\[\];','const REPLACEMENTS: &[&str] = &[\n'+''.join('    '+json.dumps(p)+',\n'for p in replaced)+'];',s);assert n==1
s,n=re.subn(r'const ADDITIONS: &\[&str\] = &\[\];','const ADDITIONS: &[&str] = &[\n'+''.join('    '+json.dumps(p)+',\n'for p in added)+'];',s);assert n==1
s,n=re.subn(r'const POLICY_LENGTH: usize = \d+;',f'const POLICY_LENGTH: usize = {len(policybytes)};',s);assert n==1
s,n=re.subn(r'const POLICY_SHA256: &str = "[0-9a-f]{64}";',f'const POLICY_SHA256: &str = "{H(policybytes)}";',s);assert n==1
anchor='        _ => None,\n    }\n}\n\n#[cfg(test)]';assert s.count(anchor)==1
arms=''.join('        '+json.dumps(p)+' => Some(include_str!("../../'+p+'")),\n'for p in newbusiness)
s=s.replace(anchor,arms+anchor);formatted=subprocess.run(['rustfmt','--edition','2021','--emit','stdout','--config','skip_children=true'],input=s.encode(),stdout=subprocess.PIPE,stderr=subprocess.PIPE,check=True).stdout;assert len(formatted.splitlines())<=1500;writes[rust]=formatted
python=E+'/overlay_page.py';s=live(python).decode()
for name,paths in [('REPLACEMENTS',replaced),('ADDITIONS',added)]:
 old=name+' = ()';assert s.count(old)==1;s=s.replace(old,name+' = '+repr(tuple(paths)))
s,n=re.subn(r'POLICY_LENGTH = \d+',f'POLICY_LENGTH = {len(policybytes)}',s);assert n==1
s,n=re.subn(r"POLICY_SHA256 = '[0-9a-f]{64}'",f"POLICY_SHA256 = '{H(policybytes)}'",s);assert n==1;writes[python]=s.encode()
cfgpath='docs/evidence/merge-2e92-browser-20261018/guard-config.json';cfg=json.loads(live(cfgpath));oldcfg=copy.deepcopy(cfg);newphysical=G('diff','--name-only','--diff-filter=A','b5a7f2ba0',head).decode().splitlines();newphysical=[p for p in newphysical if p.startswith('src/')and p.endswith('.rs')];assert len(newphysical)==14
for p in list(cfg['modules'])+newphysical:
 b=writes.get(p)or live(p);cfg['modules'][p]=dict(length=len(b),sha256=H(b))
assert list(cfg['modules'])[:166]==list(oldcfg['modules'])
for p in newphysical:assert p not in cfg['closed_prior_sources'];cfg['closed_prior_sources'].append(p)
assert cfg['closed_prior_sources'][:564]==oldcfg['closed_prior_sources']
for p,v in cfg['carriers'].items():b=live(p);v.update(length=len(b),sha256=H(b))
t=cfg['table'];b=live(t['path']);t.update(length=len(b),sha256=H(b),rows=len(b.decode().splitlines()));t['insertions']+=json.loads((D/'union-proof.json').read_text())['insertions'];expected=(R/t['prior']['archive']).read_text().splitlines()
for x in t['insertions']:assert x['path']not in expected;expected.insert(x['at'],x['path'])
assert ('\n'.join(expected)+'\n').encode()==b and t['rows']==1020
writes[cfgpath]=(json.dumps(cfg,indent=2)+'\n').encode();assert len(writes[cfgpath])<=262144
payload='docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py';s=live(payload).decode()
s,n=re.subn(r'_CONFIG_LENGTH=\d+',f'_CONFIG_LENGTH={len(writes[cfgpath])}',s);assert n==1
s,n=re.subn(r"_CONFIG_SHA256='[0-9a-f]{64}'",f"_CONFIG_SHA256='{H(writes[cfgpath])}'",s);assert n==1
s,n=re.subn(r"program=bounded\(R/'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py', \d+, '[0-9a-f]{64}'\)",f"program=bounded(R/'{python}', {len(writes[python])}, '{H(writes[python])}')",s);assert n==1;writes[payload]=s.encode();assert len(writes[payload])<=65536
# Original public/three bootstrap producers are unchanged beyond their explicit guard pin pair.
boots=['docs/evidence/accepted-strict-guard-generation-20261017/check-source.py','docs/evidence/accepted-final6-source-binding-20261017/check-source.py','docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py']
oldguard=G('show','b5a7f2ba0:'+payload);oldlen=str(len(oldguard));oldsha=H(oldguard);bootpaths=[]
for p in boots:
 b=live(p);text=b.decode();assert text.count(oldsha)==1;assert oldlen in text;text=text.replace(oldsha,H(writes[payload]))
 matches=list(re.finditer(r'(?<!\d)'+oldlen+r'(?!\d)',text));assert len(matches)==1,(p,len(matches));m=matches[0];text=text[:m.start()]+str(len(writes[payload]))+text[m.end():];writes[p]=text.encode();bootpaths.append(p)
assert len(bootpaths)==3,bootpaths
# Each before image and immutable metadata channel remains authentic.
for row in before['rows']:
 for pin in [row['current'],*row['views']]:
  b=(R/pin['archive']).read_bytes();assert(len(b),H(b))==(pin['length'],pin['sha256'])
summary=dict(head=head,old_rows=519,logical_rows=len(rows),replacements=len(replaced),additions=len(added),new_audit_leaves=len(newrust),new_audit_edges=len(edges),old_fragment_edges=10,raw_modules=len(cfg['modules']),closed_sources=len(cfg['closed_prior_sources']),ordered_rows=t['rows'],page_length=len(pagebytes),policy_length=len(policybytes),root_lines=len(live('src/source_reflows_merge/main_e177_final_inputs.rs').splitlines()),child_lines=len(writes[rust].splitlines()),replacement_sources=replaced,addition_sources=added,bootstraps=bootpaths,files=[dict(path=p,before_length=len(live(p)),before_sha256=H(live(p)),length=len(b),sha256=H(b))for p,b in writes.items()],archives=len(archives),Cargo=False)
O=D/'page-candidate';O.mkdir(exist_ok=True)
for p,b in {**archives,**writes}.items():q=O/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b)
(D/'binding-summary.json').write_text(json.dumps(summary,indent=2)+'\n');(D/'logical-current.json').write_bytes(canon(logical))
if a.write:
 assert G('rev-parse','HEAD').decode().strip()==head and not G('status','--porcelain').strip()
 for p,b in held.items():assert(R/p).read_bytes()==b
 B=R/E/'binding-before';B.mkdir(exist_ok=True)
 for p in writes:q=B/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(live(p))
 for p,b in archives.items():
  q=R/p;q.parent.mkdir(parents=True,exist_ok=True)
  if q.exists():assert q.read_bytes()==b
  else:
   with q.open('xb')as f:f.write(b)
 for p,b in writes.items():(R/p).write_bytes(b)
 (R/E/'binding-summary.json').write_text(json.dumps(summary,indent=2)+'\n')
print(json.dumps({k:v for k,v in summary.items()if k not in ['files','replacement_sources','addition_sources']},indent=2))
