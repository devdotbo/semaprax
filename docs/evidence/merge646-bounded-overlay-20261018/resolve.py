from pathlib import Path
import subprocess,json,hashlib,copy,re,difflib
R=Path('/Volumes/Semaprax/worktrees/next-large-combined-20261017');D=Path(__file__).parent;E='docs/evidence/merge646-bounded-overlay-20261018';G=lambda *a:subprocess.check_output(['git',*a],cwd=R);H=lambda b:hashlib.sha256(b).hexdigest();C=lambda v:(json.dumps(v,sort_keys=True,ensure_ascii=False,separators=(',',':'))+'\n').encode();ours=G('rev-parse','HEAD').decode().strip();theirs=G('rev-parse','MERGE_HEAD').decode().strip();assert ours=='dc36922792196bceee223c9769a0bfdaabf5341c'and theirs.startswith('646528474');conflicts=G('diff','--name-only','--diff-filter=U').decode().splitlines();assert len(conflicts)==9
writes={};snap={};archives={}
def held(p):
 if p not in snap:snap[p]=(R/p).read_bytes()
 return snap[p]
def git(ref,p):return G('show',ref+':'+p)
def archive(b):
 p=E+'/whole/'+H(b)+'.txt';archives[p]=b;return dict(length=len(b),sha256=H(b),archive=p)
for p in conflicts:
 archive(git(ours,p));archive(git(theirs,p));writes[p]=git(ours,p)
extra=['src/source_reflows_merge/main_e177_final_inputs.rs','docs/evidence/fmt-local-441-20261018/extension.json','docs/evidence/clippy-11a0-integration/inputs.json','docs/evidence/short-plan-clone-20261018/guard-config.json']
for p in extra:archive(git(ours,p)if p!='docs/evidence/short-plan-clone-20261018/guard-config.json'else git(theirs,p));archive(git(theirs,p))
for p in extra[:3]:writes[p]=git(ours,p) # exact immutable519 predecessor / original121 loader remains owning before-stage
# Entire incoming logical generation retains every unknown field, view and alias.
fmtpath='docs/evidence/main-e177-final-source-binding-20261017/current-inputs.json'
# The actual immutable433 physical packet is named by incoming CONFIG, never guessed.
icfg=json.loads(git(theirs,'docs/evidence/short-plan-clone-20261018/guard-config.json'));fmt=json.loads(git(theirs,icfg['packet']['path']));iext=json.loads(git(theirs,icfg['extension']['path']));incoming=copy.deepcopy(fmt);positions={r['source']:i for i,r in enumerate(incoming['rows'])}
for row in iext['rows']:
 if row['source']in positions:incoming['rows'][positions[row['source']]]=row
 else:positions[row['source']]=len(incoming['rows']);incoming['rows'].append(row)
assert len(incoming['rows'])==519;archive(C(incoming));istage=json.loads(git(theirs,icfg['source_successor']['path']));assert len(istage['rows'])==121
P='docs/evidence/finite-source-overlay-v2-20261018/';page=json.loads(held(P+'page.json'));policy=json.loads(held(P+'policy.json'));beforepath=Path('/Volumes/Semaprax/provenance/focused146-overlay-binding-scope-20261018/whole519-b5a7-canonical.json');base=json.loads(beforepath.read_bytes());by={r['source']:copy.deepcopy(r)for r in base['rows']};by.update({r['source']:copy.deepcopy(r)for r in page['replacements']+page['additions']});oldnames=policy['replacements'];updated=set(oldnames);imported=[];rawchanged=[]
def add_view(row,pin,name=None):
 v=copy.deepcopy(pin)
 if 'name'not in v:v['name']=name
 assert v['name']
 if v not in row['views']:
  row['views'].append(v);imported.append(dict(source=row['source'],view=v));updated.add(row['source'])
for old in incoming['rows']:
 if old['source']not in by:raise AssertionError(('incoming newlogical source',old['source']))
 row=by[old['source']]
 for v in old['views']:add_view(row,v)
 if old['current']!=row['current']:add_view(row,old['current'],'whole-incoming646-current')
for old in istage['rows']:
 if old['source']not in by:continue
 row=by[old['source']]
 # Only affected/incoming-changed rows need stage interception. All121 stages
 # are retained whole in the immutable incoming metadata archive regardless.
 if old['source']not in updated:continue
 for pin in [old['before'],old['current'],*old.get('views',[])]:
  choices=[row['current'],*row['views']]
  if not any((p['length'],p['sha256'])==(pin['length'],pin['sha256'])for p in choices):add_view(row,pin,'whole-incoming646-stage')
for source,row in by.items():
 b=writes.get(source)or held(source);pin=row['current']
 if(len(b),H(b))!=(pin['length'],pin['sha256']):
  rawchanged.append(source)
  if source in {r['source']for r in base['rows']}:
   if source not in oldnames:add_view(row,pin,'whole-pre-overlay-v2-current')
   else:add_view(row,pin,'whole-before-merge646-current')
   updated.add(source)
  else:raise AssertionError(('unexpected newmodule drift',source))
  row['current']=archive(b)
names=[r['source']for r in base['rows']if r['source']in updated];page['replacements']=[by[p]for p in names];page['additions']=[by[r['source']]for r in page['additions']]
for row in page['replacements']+page['additions']:
 if'fragments'in row:row['fragments']=[copy.deepcopy(by[x['source']])for x in row['fragments']]
pageb=C(page);assert len(pageb)<=262144;policy['replacements']=names;policy['page'].update(length=len(pageb),sha256=H(pageb));pol=C(policy);writes[P+'page.json']=pageb;writes[P+'policy.json']=pol
ns={};exec(compile(held(P+'overlay_page.py'),P+'overlay_page.py','exec'),ns);logical=ns['compose_values'](base,policy,page);assert len(logical['rows'])==542
# Literal scope and policy seals only. Loader algorithms remain byteexact.
rust='src/source_reflows_merge/main_e177_overlay_v2.rs';s=held(rust).decode();s,n=re.subn(r'const REPLACEMENTS: &\[&str\] = &\[.*?\];','const REPLACEMENTS: &[&str] = &[\n'+''.join('    '+json.dumps(p)+',\n'for p in names)+'];',s,count=1,flags=re.S);assert n==1;s,n=re.subn(r'const POLICY_LENGTH: usize = \d+;',f'const POLICY_LENGTH: usize = {len(pol)};',s);assert n==1;s,n=re.subn(r'const POLICY_SHA256: &str = "[0-9a-f]{64}";',f'const POLICY_SHA256: &str = "{H(pol)}";',s);assert n==1;writes[rust]=subprocess.run(['rustfmt','--edition','2021','--emit','stdout','--config','skip_children=true'],input=s.encode(),stdout=subprocess.PIPE,check=True).stdout;assert len(writes[rust].splitlines())<=1500
python=P+'overlay_page.py';s=held(python).decode();s,n=re.subn(r'^REPLACEMENTS = .*$','REPLACEMENTS = '+repr(tuple(names)),s,flags=re.M);assert n==1;s,n=re.subn(r'POLICY_LENGTH = \d+',f'POLICY_LENGTH = {len(pol)}',s);assert n==1;s,n=re.subn(r"POLICY_SHA256 = '[0-9a-f]{64}'",f"POLICY_SHA256 = '{H(pol)}'",s);assert n==1;writes[python]=s.encode()
cfgp='docs/evidence/merge-2e92-browser-20261018/guard-config.json';cfg=json.loads(held(cfgp));assert len(cfg['modules'])==180
for p in cfg['modules']:
 b=writes.get(p)or held(p);cfg['modules'][p]=dict(length=len(b),sha256=H(b))
writes[cfgp]=(json.dumps(cfg,indent=2)+'\n').encode();payload='docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py';s=git(ours,payload).decode();s,n=re.subn(r'_CONFIG_LENGTH=\d+',f'_CONFIG_LENGTH={len(writes[cfgp])}',s);assert n==1;s,n=re.subn(r"_CONFIG_SHA256='[0-9a-f]{64}'",f"_CONFIG_SHA256='{H(writes[cfgp])}'",s);assert n==1;s,n=re.subn(r"program=bounded\(R/'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py', \d+, '[0-9a-f]{64}'\)",f"program=bounded(R/'{python}', {len(writes[python])}, '{H(writes[python])}')",s);assert n==1;writes[payload]=s.encode()
for p in ['docs/evidence/accepted-strict-guard-generation-20261017/check-source.py','docs/evidence/accepted-final6-source-binding-20261017/check-source.py','docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py']:
 s=git(ours,p).decode();old=git(ours,payload);assert s.count(H(old))==1;s=s.replace(H(old),H(writes[payload]));matches=list(re.finditer(r'(?<!\d)'+str(len(old))+r'(?!\d)',s));assert len(matches)==1;m=matches[0];s=s[:m.start()]+str(len(writes[payload]))+s[m.end():];writes[p]=s.encode()
# Retain complete342-owner models. Everyoldchain keeps exactroot/kind/pkg/text;
# onlyactual source hashes and unique physical declaration offsets may move.
modelchanges=[];edgechanges=[];incoming_extra=[]
def shape(c):
 result={**c,'edges':[{k:v for k,v in e.items()if k!='start'}for e in c['edges']]}
 for e in result['edges']:
  if (e['parent'],e['child'],e['text'])==('src/cleanup_plan/replay/inventory_coverage.rs','src/cleanup_plan/replay/inventory_coverage/paid_before.rs','#[cfg(test)]\nmod paid_before;'):
   text='#[cfg(test)]\npub(super) mod paid_before;';assert held(e['parent']).count(text.encode())==1 and e['attributes']==['#[cfg(test)]'];e['text']=text
 return result
for name in ['hir','projection']:
 p='docs/evidence/a3-e47-final-custody-20261016/'+name+'.json';m=json.loads(git(ours,p));oldm=copy.deepcopy(m);im=json.loads(git(theirs,p))
 for source,value in im['classes'].items():
  assert source in m['classes']and value['role']==m['classes'][source]['role']
  ours_shapes=[shape(c)for c in m['classes'][source]['chains']]
  for c in value['chains']:
   if shape(c)not in ours_shapes:incoming_extra.append(dict(domain=name,source=source,chain=c))
 assert not incoming_extra,incoming_extra
 for source,pin in m['sources'].items():
  b=writes.get(source)or held(source)
  if(pin['length'],pin['sha256'])!=(len(b),H(b)):modelchanges.append(dict(domain=name,source=source,before=copy.deepcopy(pin),after=dict(length=len(b),sha256=H(b))))
  pin.update(length=len(b),sha256=H(b))
 oldcache={}
 for source,value in m['classes'].items():
  for chain in value['chains']:
   for edge in chain['edges']:
    pth=edge['parent'];text=edge['text'].encode();raw=writes.get(pth)or held(pth)
    if raw[edge['start']:edge['start']+len(text)]==text:continue
    if pth not in oldcache:oldcache[pth]=git(ours,pth)
    old=oldcache[pth];assert old[edge['start']:edge['start']+len(text)]==text
    found=[x.start()for x in re.finditer(re.escape(text),raw)];assert len(found)==1,(pth,edge['text']);oldat=edge['start'];edge['start']=found[0];edgechanges.append(dict(domain=name,parent=pth,child=edge['child'],before=oldat,after=found[0],text=edge['text']))
 # Exactclass/role/chunk/prefix inverse afterrestoringonlyedgeoffsets/sources.
 restored=copy.deepcopy(m);restored['sources']=oldm['sources']
 for source,value in restored['classes'].items():
  for ci,chain in enumerate(value['chains']):
   for ei,edge in enumerate(chain['edges']):edge['start']=oldm['classes'][source]['chains'][ci]['edges'][ei]['start']
 assert restored==oldm
 writes[p]=(json.dumps(m,indent=2)+'\n').encode()
ownp='docs/evidence/a3-e47-final-custody-20261016/new-owners.json';owners=json.loads(git(ours,ownp));assert len(owners)==342
for row in owners:
 b=writes.get(row['source'])or held(row['source']);row.update(sha256=H(b),lines=len(b.splitlines()))
writes[ownp]=(json.dumps(owners,indent=2)+'\n').encode()
# Incoming/ours boundary reports remain fullhistoricalgenerations. Activeproof
# is intentionallypending fresh originalgenerator at themergedHEAD.
for p in ['docs/evidence/source-boundary-domains-20261016/current-domains.json','docs/evidence/source-boundary-domains-20261016/boundary-source-proof.json']:writes[p]=git(ours,p)
for p,b in writes.items():held(p)
assert G('rev-parse','HEAD').decode().strip()==ours and G('rev-parse','MERGE_HEAD').decode().strip()==theirs
for p,b in snap.items():assert(R/p).read_bytes()==b,p
for p,b in archives.items():
 q=R/p;q.parent.mkdir(parents=True,exist_ok=True)
 if q.exists():assert not q.is_symlink()and q.read_bytes()==b
 else:
  with q.open('xb')as f:f.write(b)
for p,b in writes.items():(R/p).write_bytes(b)
summary=dict(status='MERGE_SOURCE_CHECKPOINT_PENDING_RUST_AND_BOUNDARY_REQUALIFICATION',ours=ours,incoming=theirs,resolved=conflicts,old_extension_retained=True,incoming_extension_whole_archived=True,incoming_stage121_whole_archived=True,rawchanged=rawchanged,imported_views=imported,logical_rows=542,raw_modules=180,ordered_rows=1020,owners=342,page_length=len(pageb),old519_inverse=True,model_source_changes=modelchanges,typed_edge_offset_changes=edgechanges,incoming_extra_chains=incoming_extra,fullarchives=[dict(path=p,length=len(b),sha256=H(b))for p,b in archives.items()],files=[dict(path=p,length=len(b),sha256=H(b))for p,b in writes.items()],Cargo=False)
q=R/E;q.mkdir(exist_ok=True);(q/'merge-source-receipt.json').write_text(json.dumps(summary,indent=2)+'\n');(q/'README.md').write_text('Merged source checkpoint. Both complete metadata generations and all incoming source/evidence are retained. The bounded page owns current merged source; the complete original 519-row predecessor and original 121-row stage remain immutable. Current source producers, boundary proof and Rust build/runtime qualification remain pending for this merged generation. Prior-head source or runtime evidence is not a current-head PASS.\n');review=Path('/Volumes/Semaprax/provenance/dc369-shortplan-auto-merge-independent-review-20261018/source-review.json');(q/'independent-source-review.json').write_bytes(review.read_bytes());(D/'receipt.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps({k:v for k,v in summary.items()if k in ['status','logical_rows','raw_modules','ordered_rows','owners','page_length','rawchanged','incoming_extra_chains']},indent=2))
