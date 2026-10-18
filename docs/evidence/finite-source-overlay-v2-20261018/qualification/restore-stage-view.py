from pathlib import Path
import subprocess,json,hashlib,copy,re
R=Path('/Volumes/Semaprax/worktrees/focused146-structural-repairs-20261018');D=Path(__file__).parent;E='docs/evidence/finite-source-overlay-v2-20261018';H=lambda b:hashlib.sha256(b).hexdigest();C=lambda v:(json.dumps(v,sort_keys=True,ensure_ascii=False,separators=(',',':'))+'\n').encode();G=lambda *a:subprocess.check_output(['git',*a],cwd=R);head=G('rev-parse','HEAD').decode().strip();assert not G('status','--porcelain').strip();held={};writes={}
def read(p):
 if p not in held:
  b=(R/p).read_bytes();assert not(R/p).is_symlink()and b==G('show',head+':'+p);held[p]=b
 return held[p]
payload='docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py';ns={'__file__':str(R/payload),'_AUTHENTICATED_PROGRAM':read(payload)};exec(compile(read(payload),str(R/payload),'exec'),ns);rows=ns['_successor_rows']();logical=ns['packet']();by={r['source']:r for r in logical['rows']};page=json.loads(read(E+'/page.json'));policy=json.loads(read(E+'/policy.json'));affected=set(policy['replacements'])|{r['source']for r in page['additions']};missing=[]
for row in rows:
 if row['source']not in affected:continue
 pins={(p['length'],p['sha256'])for p in [by[row['source']]['current'],*by[row['source']]['views']]}
 for pin in [row['before'],row['current'],*row.get('views',[])]:
  b=read(pin['archive']);assert(len(b),H(b))==(pin['length'],pin['sha256'])
  if(pin['length'],pin['sha256'])not in pins:missing.append((row['source'],pin));pins.add((pin['length'],pin['sha256']))
expected={
 'src/source_reflows/incoming090_semantic_receivers.rs':dict(name='whole-12ad-current',length=49904,sha256='715ffa67a02985b31228264a07b0b81e46307e80845b319d5ba9a1cdf024156a',archive='docs/evidence/clippy-4a6eb/cV_6Z6AphbMSKCZKB7C4HkYwfoCEWzGdW6mhzfAkFWo.txt'),
 'src/cleanup_plan/replay/expression_facts/receiving/retained/case_certified/lookup_tests.rs':dict(name='whole-local-a5c804-current',length=11205,sha256='0e78dbe7a5413c43cbf87f087508b89822b57c28cf67b0ea648fd309815fc0ab',archive='docs/evidence/clippy-4a6eb/Dnjb56VBPEPL-H8IdQi4mCK1fCjPZ7DqZI_TCYFfwKs.txt')}
assert dict(missing)==expected,missing
for source,pin in missing:
 row=next(r for r in page['replacements']if r['source']==source);row['views'].append(copy.deepcopy(pin))
pb=C(page);assert len(pb)<=262144;policy['page'].update(length=len(pb),sha256=H(pb));pol=C(policy);writes[E+'/page.json']=pb;writes[E+'/policy.json']=pol
for p in ['src/source_reflows_merge/main_e177_overlay_v2.rs',E+'/overlay_page.py']:
 s=read(p).decode()
 if p.endswith('.rs'):
  s,n=re.subn(r'const POLICY_LENGTH: usize = \d+;',f'const POLICY_LENGTH: usize = {len(pol)};',s);assert n==1;s,n=re.subn(r'const POLICY_SHA256: &str = "[0-9a-f]{64}";',f'const POLICY_SHA256: &str = "{H(pol)}";',s);assert n==1
 else:
  s,n=re.subn(r'POLICY_LENGTH = \d+',f'POLICY_LENGTH = {len(pol)}',s);assert n==1;s,n=re.subn(r"POLICY_SHA256 = '[0-9a-f]{64}'",f"POLICY_SHA256 = '{H(pol)}'",s);assert n==1
 writes[p]=s.encode()
cfgp='docs/evidence/merge-2e92-browser-20261018/guard-config.json';cfg=json.loads(read(cfgp));p='src/source_reflows_merge/main_e177_overlay_v2.rs';cfg['modules'][p].update(length=len(writes[p]),sha256=H(writes[p]));writes[cfgp]=(json.dumps(cfg,indent=2)+'\n').encode();s=read(payload).decode();s,n=re.subn(r'_CONFIG_LENGTH=\d+',f'_CONFIG_LENGTH={len(writes[cfgp])}',s);assert n==1;s,n=re.subn(r"_CONFIG_SHA256='[0-9a-f]{64}'",f"_CONFIG_SHA256='{H(writes[cfgp])}'",s);assert n==1;p=E+'/overlay_page.py';s,n=re.subn(r"program=bounded\(R/'docs/evidence/finite-source-overlay-v2-20261018/overlay_page.py', \d+, '[0-9a-f]{64}'\)",f"program=bounded(R/'{p}', {len(writes[p])}, '{H(writes[p])}')",s);assert n==1;writes[payload]=s.encode()
for p in ['docs/evidence/accepted-strict-guard-generation-20261017/check-source.py','docs/evidence/accepted-final6-source-binding-20261017/check-source.py','docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py']:
 s=read(p).decode();assert s.count(H(read(payload)))==1;s=s.replace(H(read(payload)),H(writes[payload]));matches=list(re.finditer(r'(?<!\d)'+str(len(read(payload)))+r'(?!\d)',s));assert len(matches)==1;m=matches[0];s=s[:m.start()]+str(len(writes[payload]))+s[m.end():];writes[p]=s.encode()
assert G('rev-parse','HEAD').decode().strip()==head and not G('status','--porcelain').strip()
for p,b in held.items():assert(R/p).read_bytes()==b
B=R/E/'stage-view-before';B.mkdir()
for p,b in writes.items():q=B/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(read(p));(R/p).write_bytes(b)
summary=dict(status='TWO_AUTHENTIC_STAGE_VIEWS_RESTORED',head=head,views=expected,page_length=len(pb),files=[dict(path=p,before_length=len(read(p)),before_sha256=H(read(p)),length=len(b),sha256=H(b))for p,b in writes.items()],old_source_raw_unchanged=True)
(R/E/'stage-view-restoration.json').write_text(json.dumps(summary,indent=2)+'\n');print(json.dumps(summary,indent=2))
