from pathlib import Path
import subprocess,json,hashlib,copy,re
R=Path('/Volumes/Semaprax/worktrees/focused146-structural-repairs-20261018');D=Path(__file__).parent;E='docs/evidence/finite-source-overlay-v2-20261018';G=lambda *x:subprocess.check_output(['git',*x],cwd=R);H=lambda b:hashlib.sha256(b).hexdigest();C=lambda v:(json.dumps(v,sort_keys=True,ensure_ascii=False,separators=(',',':'))+'\n').encode();head=G('rev-parse','HEAD').decode().strip();assert not G('status','--porcelain').strip();held={};writes={};archives={}
def read(p):
 if p not in held:
  q=R/p;assert q.is_file()and not q.is_symlink();b=q.read_bytes();assert b==G('show',head+':'+p);held[p]=b
 return held[p]
def image(b):
 p='e/'+H(b);archives[p]=b;return dict(length=len(b),sha256=H(b),archive=p)
page=json.loads(read(E+'/page.json'));policy=json.loads(read(E+'/policy.json'));base=json.loads(Path('/Volumes/Semaprax/provenance/focused146-overlay-binding-scope-20261018/whole519-b5a7-canonical.json').read_bytes());by={r['source']:copy.deepcopy(r)for r in base['rows']};by.update({r['source']:copy.deepcopy(r)for r in page['replacements']+page['additions']});changed=[]
for p,row in by.items():
 b=read(p);pin=row['current']
 if(len(b),H(b))!=(pin['length'],pin['sha256']):
  changed.append(p);oldbase=next((r for r in base['rows']if r['source']==p),None);assert oldbase
  if p not in policy['replacements']:row['views'].append(dict(name='whole-pre-overlay-v2-current',**pin))
  else:row['views'].append(dict(name='whole-before-genuine174-FINAL6-overlay',**pin))
  row['current']=image(b)
assert set(changed)=={'src/source_reflows/incoming090_semantic_receivers.rs','src/source_reflows_merge/ordinary_plan_inputs.rs'}
oldnames=policy['replacements'];names=[r['source']for r in base['rows']if r['source']in set(oldnames+changed)];page['replacements']=[by[p]for p in names]
for row in page['replacements']:
 if'fragments'in row:row['fragments']=[copy.deepcopy(by[x['source']])for x in row['fragments']]
policy['replacements']=names;pageb=C(page);assert len(pageb)<=262144;policy['page'].update(length=len(pageb),sha256=H(pageb));pb=C(policy);writes[E+'/page.json']=pageb;writes[E+'/policy.json']=pb
ns={};exec(compile(read(E+'/overlay_page.py'),E+'/overlay_page.py','exec'),ns);logical=ns['compose_values'](base,policy,page);assert len(logical['rows'])==542
rust='src/source_reflows_merge/main_e177_overlay_v2.rs';s=read(rust).decode();pattern=r'const REPLACEMENTS: &\[&str\] = &\[.*?\];';s,n=re.subn(pattern,'const REPLACEMENTS: &[&str] = &[\n'+''.join('    '+json.dumps(p)+',\n'for p in names)+'];',s,count=1,flags=re.S);assert n==1
s,n=re.subn(r'const POLICY_LENGTH: usize = \d+;',f'const POLICY_LENGTH: usize = {len(pb)};',s);assert n==1;s,n=re.subn(r'const POLICY_SHA256: &str = "[0-9a-f]{64}";',f'const POLICY_SHA256: &str = "{H(pb)}";',s);assert n==1;writes[rust]=subprocess.run(['rustfmt','--edition','2021','--emit','stdout','--config','skip_children=true'],input=s.encode(),stdout=subprocess.PIPE,check=True).stdout;assert len(writes[rust].splitlines())<=1500
python=E+'/overlay_page.py';s=read(python).decode();s,n=re.subn(r'^REPLACEMENTS = .*$', 'REPLACEMENTS = '+repr(tuple(names)),s,flags=re.M);assert n==1;s,n=re.subn(r'POLICY_LENGTH = \d+',f'POLICY_LENGTH = {len(pb)}',s);assert n==1;s,n=re.subn(r"POLICY_SHA256 = '[0-9a-f]{64}'",f"POLICY_SHA256 = '{H(pb)}'",s);assert n==1;writes[python]=s.encode()
cfgp='docs/evidence/merge-2e92-browser-20261018/guard-config.json';cfg=json.loads(read(cfgp));oldcfg=copy.deepcopy(cfg)
for p in cfg['modules']:
 b=writes.get(p)or read(p);cfg['modules'][p]=dict(length=len(b),sha256=H(b))
assert len(cfg['modules'])==180 and cfg['closed_prior_sources']==oldcfg['closed_prior_sources'];writes[cfgp]=(json.dumps(cfg,indent=2)+'\n').encode()
payload='docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py';s=read(payload).decode();s,n=re.subn(r'_CONFIG_LENGTH=\d+',f'_CONFIG_LENGTH={len(writes[cfgp])}',s);assert n==1;s,n=re.subn(r"_CONFIG_SHA256='[0-9a-f]{64}'",f"_CONFIG_SHA256='{H(writes[cfgp])}'",s);assert n==1;s,n=re.subn(r"program=bounded\(R/'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py', \d+, '[0-9a-f]{64}'\)",f"program=bounded(R/'{python}', {len(writes[python])}, '{H(writes[python])}')",s);assert n==1;writes[payload]=s.encode()
boots=['docs/evidence/accepted-strict-guard-generation-20261017/check-source.py','docs/evidence/accepted-final6-source-binding-20261017/check-source.py','docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py']
for p in boots:
 s=read(p).decode();assert s.count(H(read(payload)))==1;s=s.replace(H(read(payload)),H(writes[payload]));matches=list(re.finditer(r'(?<!\d)'+str(len(read(payload)))+r'(?!\d)',s));assert len(matches)==1;m=matches[0];s=s[:m.start()]+str(len(writes[payload]))+s[m.end():];writes[p]=s.encode()
assert G('rev-parse','HEAD').decode().strip()==head and not G('status','--porcelain').strip()
for p,b in held.items():assert(R/p).read_bytes()==b
B=R/E/'postbind-before';B.mkdir()
for p,b in writes.items():q=B/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(read(p))
for p,b in archives.items():
 q=R/p;q.parent.mkdir(parents=True,exist_ok=True)
 if q.exists():assert not q.is_symlink()and q.read_bytes()==b
 else:
  with q.open('xb')as f:f.write(b)
for p,b in writes.items():(R/p).write_bytes(b)
summary=dict(status='SOURCE_ONLY_TWO_ACTUAL_PRODUCER_OUTPUTS_RESEALED',head=head,changed_raw_sources=changed,page_length=len(pageb),policy_length=len(pb),rows=542,replacements=len(names),additions=len(page['additions']),whole_before_retained=True,original519_inverse=True,unchanged_old_extension=True,files=[dict(path=p,before_length=len(read(p)),before_sha256=H(read(p)),length=len(b),sha256=H(b))for p,b in writes.items()],producer_receipts=dict(actual174=json.loads((D/'actual174-terminal.json').read_bytes()),actualF6=json.loads((D/'actualF6-terminal.json').read_bytes())),Rust_executed=False)
(R/E/'postbind-receipt.json').write_text(json.dumps(summary,indent=2)+'\n');(D/'logical-postbind.json').write_bytes(C(logical));print(json.dumps({k:v for k,v in summary.items()if k!='files'},indent=2))
