from pathlib import Path
import subprocess,json,hashlib,copy,re,os
R=Path('/Volumes/Semaprax/worktrees/next-large-combined-20261017');D=Path(__file__).parent;E='docs/evidence/v010-5075-four-source-reseal-20261018';P='docs/evidence/finite-source-overlay-v2-20261018/';G=lambda *a:subprocess.check_output(['git',*a],cwd=R);H=lambda b:hashlib.sha256(b).hexdigest();C=lambda v:(json.dumps(v,sort_keys=True,ensure_ascii=False,separators=(',',':'))+'\n').encode();head=G('rev-parse','HEAD').decode().strip();assert head=='5afdbd78a9d2bf6b77f7ce3d67b874288ac487d3';base=G('rev-parse','5075ead').decode().strip();held={};writes={};archives={};allowed={'src/cleanup_plan/replay/ordinary_receiving.rs','src/cleanup_plan/replay/path_join/selected.rs','src/cleanup_plan/replay/path_join/selected/walk.rs','src/cleanup_plan/replay/structural_control/selected.rs'}
def read(p):
 if p not in held:
  q=R/p;assert q.is_file()and not q.is_symlink();held[p]=q.read_bytes()
 return held[p]
def image(b):
 p='e/'+H(b);archives[p]=b;return dict(length=len(b),sha256=H(b),archive=p)
payload='docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py';program=read(payload);ns={'__file__':str(R/payload),'_AUTHENTICATED_PROGRAM':program};exec(compile(program,str(R/payload),'exec'),ns);logical_before=ns['packet']();by={r['source']:copy.deepcopy(r)for r in logical_before['rows']};page=json.loads(read(P+'page.json'));policy=json.loads(read(P+'policy.json'));changed=[]
for p,row in by.items():
 raw=read(p);pin=row['current']
 if(len(raw),H(raw))!=(pin['length'],pin['sha256']):
  changed.append(p);assert p in allowed;prior=G('show',base+':'+p);assert(len(prior),H(prior))==(pin['length'],pin['sha256']);row['views'].append(dict(name='whole-5075-before-compile-repair',**pin));
  if p not in policy['replacements']and p not in {r['source']for r in page['additions']}:row['views'].append(dict(name='whole-pre-overlay-v2-current',**pin))
  row['current']=image(raw)
assert set(changed)==allowed
oldnames=set(policy['replacements']);oldbase=ns['packet_before_overlay']();names=[r['source']for r in oldbase['rows']if r['source']in oldnames|set(changed)];policy['replacements']=names;page['replacements']=[by[p]for p in names];page['additions']=[by[r['source']]for r in page['additions']]
for row in page['replacements']+page['additions']:
 if'fragments'in row:row['fragments']=[copy.deepcopy(by[x['source']])for x in row['fragments']]
pageb=C(page);assert len(pageb)<=262144;policy['page'].update(length=len(pageb),sha256=H(pageb));pol=C(policy);assert len(pol)<=262144;writes[P+'page.json']=pageb;writes[P+'policy.json']=pol
# Exactold543 object inverse, includingunknownfields/vieworder andall76edges.
result=copy.deepcopy(logical_before);positions={r['source']:i for i,r in enumerate(result['rows'])}
for row in page['replacements']+page['additions']:result['rows'][positions[row['source']]]=row
restored=copy.deepcopy(result);restored['rows']=logical_before['rows'];assert restored==logical_before
kernel={};exec(compile(read(P+'overlay_page.py'),P+'overlay_page.py','exec'),kernel);original519=json.loads(Path('/Volumes/Semaprax/provenance/focused146-overlay-binding-scope-20261018/whole519-b5a7-canonical.json').read_bytes());assert kernel['compose_values'](original519,policy,page)==result and len(result['rows'])==543
rust='src/source_reflows_merge/main_e177_overlay_v2.rs';python=P+'overlay_page.py'
for p in [rust,python]:
 s=read(p).decode()
 if p==rust:
  s,n=re.subn(r'const REPLACEMENTS: &\[&str\] = &\[.*?\];','const REPLACEMENTS: &[&str] = &[\n'+''.join('    '+json.dumps(x)+',\n'for x in names)+'];',s,count=1,flags=re.S);assert n==1
  s,n=re.subn(r'const POLICY_LENGTH: usize = \d+;',f'const POLICY_LENGTH: usize = {len(pol)};',s);assert n==1;s,n=re.subn(r'const POLICY_SHA256: &str = "[0-9a-f]{64}";',f'const POLICY_SHA256: &str = "{H(pol)}";',s);assert n==1
 else:
  s,n=re.subn(r'^REPLACEMENTS = .*$', 'REPLACEMENTS = '+repr(tuple(names)),s,flags=re.M);assert n==1
  s,n=re.subn(r'POLICY_LENGTH = \d+',f'POLICY_LENGTH = {len(pol)}',s);assert n==1;s,n=re.subn(r"POLICY_SHA256 = '[0-9a-f]{64}'",f"POLICY_SHA256 = '{H(pol)}'",s);assert n==1
 writes[p]=s.encode()
cfgp='docs/evidence/merge-2e92-browser-20261018/guard-config.json';cfg=json.loads(read(cfgp));oldcfg=copy.deepcopy(cfg)
for p in cfg['modules']:
 raw=writes.get(p)or read(p);cfg['modules'][p].update(length=len(raw),sha256=H(raw))
assert len(cfg['modules'])==180 and cfg['closed_prior_sources']==oldcfg['closed_prior_sources']and cfg['table']==oldcfg['table'];writes[cfgp]=(json.dumps(cfg,indent=2)+'\n').encode();s=read(payload).decode();s,n=re.subn(r'_CONFIG_LENGTH=\d+',f'_CONFIG_LENGTH={len(writes[cfgp])}',s);assert n==1;s,n=re.subn(r"_CONFIG_SHA256='[0-9a-f]{64}'",f"_CONFIG_SHA256='{H(writes[cfgp])}'",s);assert n==1;s,n=re.subn(r"program=bounded\(R/'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py', \d+, '[0-9a-f]{64}'\)",f"program=bounded(R/'{python}', {len(writes[python])}, '{H(writes[python])}')",s);assert n==1;writes[payload]=s.encode()
for p in ['docs/evidence/accepted-strict-guard-generation-20261017/check-source.py','docs/evidence/accepted-final6-source-binding-20261017/check-source.py','docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py']:
 s=read(p).decode();assert s.count(H(read(payload)))==1;s=s.replace(H(read(payload)),H(writes[payload]));hits=list(re.finditer(r'(?<!\d)'+str(len(read(payload)))+r'(?!\d)',s));assert len(hits)==1;m=hits[0];s=s[:m.start()]+str(len(writes[payload]))+s[m.end():];writes[p]=s.encode()
modelchanges=[];edgechanges=[];oldblobs={}
for name in ['hir','projection']:
 p='docs/evidence/a3-e47-final-custody-20261016/'+name+'.json';m=json.loads(read(p));old=copy.deepcopy(m)
 for source,pin in m['sources'].items():
  raw=writes.get(source)or read(source)
  if(len(raw),H(raw))!=(pin['length'],pin['sha256']):
   assert source in allowed|{rust};modelchanges.append(dict(domain=name,source=source,before=copy.deepcopy(pin),after=dict(length=len(raw),sha256=H(raw))));oldblobs.setdefault(source,G('show',base+':'+source));assert(len(oldblobs[source]),H(oldblobs[source]))==(pin['length'],pin['sha256']);pin.update(length=len(raw),sha256=H(raw))
 for value in m['classes'].values():
  for chain in value['chains']:
   for e in chain['edges']:
    raw=writes.get(e['parent'])or read(e['parent']);text=e['text'].encode()
    if raw[e['start']:e['start']+len(text)]==text:continue
    assert e['parent']in allowed|{rust};oldblobs.setdefault(e['parent'],G('show',base+':'+e['parent']));b=oldblobs[e['parent']];assert b[e['start']:e['start']+len(text)]==text;hits=[x.start()for x in re.finditer(re.escape(text),raw)];assert len(hits)==1,(e['parent'],e['text']);edgechanges.append(dict(domain=name,parent=e['parent'],child=e['child'],text=e['text'],before=e['start'],after=hits[0]));e['start']=hits[0]
 inv=copy.deepcopy(m);inv['sources']=old['sources']
 for source,value in inv['classes'].items():
  for ci,c in enumerate(value['chains']):
   for ei,e in enumerate(c['edges']):e['start']=old['classes'][source]['chains'][ci]['edges'][ei]['start']
 assert inv==old;writes[p]=(json.dumps(m,indent=2)+'\n').encode()
p='docs/evidence/a3-e47-final-custody-20261016/new-owners.json';owners=json.loads(read(p));assert len(owners)==342
for row in owners:
 raw=writes.get(row['source'])or read(row['source']);row.update(sha256=H(raw),lines=len(raw.splitlines()))
writes[p]=(json.dumps(owners,indent=2)+'\n').encode()
assert G('rev-parse','HEAD').decode().strip()==head
for p,b in held.items():assert(R/p).read_bytes()==b
B=R/E/'before';B.mkdir(exist_ok=True)
for p,b in writes.items():q=B/(p.replace('/','__')+'.txt');q.write_bytes(read(p))
for p in allowed:(B/(p.replace('/','__')+'.5075.txt')).write_bytes(G('show',base+':'+p))
for p,b in archives.items():
 q=R/p
 if q.exists():assert not q.is_symlink()and q.read_bytes()==b
 else:
  with q.open('xb')as f:f.write(b)
for p,b in writes.items():(R/p).write_bytes(b)
receipt=dict(status='SOURCE_ONLY_FOUR_COMPILE_REPAIRS_RESEALED',source_head=head,failed_source_head=base,changed_sources=changed,rows=543,old_rows=543,whole_old543_object_inverse=True,whole_original519_object_inverse=True,all_prior_view_prefixes=True,modules=180,owners=342,ordered_rows=1020,page=policy['page'],model_source_changes=modelchanges,typed_edge_offset_changes=edgechanges,role_class_chunk_order_unchanged=True,files=[dict(path=p,before=dict(length=len(read(p)),sha256=H(read(p)),archive=E+'/before/'+p.replace('/','__')+'.txt'),after=dict(length=len(b),sha256=H(b)))for p,b in writes.items()],no_current_Rust_or_runtime_PASS=True,no_producer_rerun=True,Cargo=False)
(R/E/'reseal-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');(D/'receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps({k:v for k,v in receipt.items()if k in ['status','changed_sources','rows','modules','owners','ordered_rows','page']},indent=2))
