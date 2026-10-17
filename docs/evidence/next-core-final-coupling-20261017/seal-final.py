"""One closed successor: retain whole49, old54/454, and all historical view bytes."""
from pathlib import Path
import json,hashlib,subprocess,ast,os,stat,re
R=Path('/Volumes/Semaprax/worktrees/wasm-custody-compile-repair-20261017');HEAD='566bf724d583f2d00040e9336404d6d6f902bd38';ROOT='0f9fb3766f0413694e4a12447538f7c7bb99627f';AFA='afa1e4d9da88da6420776ae3184cc9efb552d022';REL='docs/evidence/next-core-final-coupling-20261017';ST='docs/evidence/main-e177-final-source-binding-20261017';H=lambda b:hashlib.sha256(b).hexdigest();G=lambda rev,p:subprocess.check_output(['git','show',rev+':'+p],cwd=R)
def held(p,cap=9753094):
 fd=os.open(R/p,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC);ident=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  st=os.fstat(fd);assert stat.S_ISREG(st.st_mode)and st.st_size<=cap
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(st.st_size);assert len(b)==st.st_size and not f.read(1)
  assert ident(st)==ident(os.fstat(fd))==ident(os.lstat(R/p));return b
 finally:os.close(fd)
NEW_ARCHIVES=set()
def archive(raw,category):
 p=ST+'/'+category+'/'+H(raw)+'.txt';q=R/p;q.parent.mkdir(parents=True,exist_ok=True)
 if q.exists():assert held(p)==raw
 else:q.write_bytes(raw);NEW_ARCHIVES.add(p)
 return dict(length=len(raw),sha256=H(raw),archive=p)
pp=ST+'/current-inputs.json';priorraw=held(pp,262144);assert priorraw==G(HEAD,pp);packet=json.loads(priorraw);assert len(packet['rows'])==49;rows={r['source']:r for r in packet['rows']};catalog={};wanted={}
def want(source,name,v,raw):
 assert(len(raw),H(raw))==(v['length'],v['sha256']),(source,name)
 wanted.setdefault(source,[]).append(dict(name=name,**archive(raw,'views')))
for row in packet['rows']:
 for v in [row['current'],*row['views']]:
  raw=held(v['archive']);assert(len(raw),H(raw))==(v['length'],v['sha256']);catalog[(row['source'],v['length'],v['sha256'])]=raw
 # Whole preceding current49 endpoint remains a named independent stage.
 want(row['source'],'whole-pre-core49-current',row['current'],catalog[(row['source'],row['current']['length'],row['current']['sha256'])])
old54=json.loads(held('docs/evidence/next-big-batch-source-binding-20261017/current-inputs.json'))
for row in old54['rows']:
 for v in [row['current'],*row['views']]:
  raw=held(v['archive']);assert(len(raw),H(raw))==(v['length'],v['sha256']);catalog[(row['source'],v['length'],v['sha256'])]=raw
 want(row['source'],'whole-afa54-current',row['current'],catalog[(row['source'],row['current']['length'],row['current']['sha256'])])
old454=json.loads(held('docs/evidence/main99f-full-controller-acquisition-20261017/inputs.json'))
for row in old454['rows']:
 source=row['path'];v=dict(length=row['current_length'],sha256=row['current_sha256']);raw=catalog.get((source,v['length'],v['sha256']))
 if raw is None:
  raw=G(AFA,source);assert(len(raw),H(raw))==(v['length'],v['sha256']),(source,'whole454')
 want(source,'whole-afa454-current',v,raw)
required=json.loads(held('docs/evidence/paid-many-source-stage-repair-20261017/required-final-views.json'))['rows']
for r in required:want(r['source'],r['name'],r,held(r['archive']))
info=json.loads(held(REL+'/raw-and-table.json'));newmods=info['new_modules'];assert len(newmods)==10
sources=set(rows)|set(newmods)
for source,views in wanted.items():
 b=held(source)
 if any((len(b),H(b))!=(v['length'],v['sha256'])for v in views):sources.add(source)
# The final reader/packet are excluded from their own source domain.
assert 'src/source_reflows_merge/main_e177_final_inputs.rs'not in sources
extra=[]
for source in sorted(sources):
 b=held(source);current=archive(b,'current');row=rows.get(source,dict(source=source,compiled=source.endswith('.rs'),views=[]));seen={(v['length'],v['sha256'])for v in row['views']}
 for v in wanted.get(source,[]):
  key=(v['length'],v['sha256'])
  if key not in seen and key!=(current['length'],current['sha256']):row['views'].append(v);seen.add(key)
 row['current']=current
 if source not in rows:extra.append(row)
packet['rows']=[rows[r['source']]for r in packet['rows']]+extra
packet['semantic_head']=HEAD;packet['current_stage']=dict(kind='reviewed-working-stage',git_head=HEAD,actual_current_tuples=[dict(source=r['source'],length=r['current']['length'],sha256=r['current']['sha256'])for r in packet['rows']])
for a in packet['complete_old_table_anchors']:
 b=held(a['path']);assert(len(b),H(b))==(a['length'],a['sha256']);assert held(a['archive'])==b
raw=(json.dumps(packet,indent=2)+'\n').encode();assert len(raw)<=262144,len(raw);(R/pp).write_bytes(raw)
used={v['archive']for row in packet['rows']for v in [row['current'],*row['views']]}|{v['archive']for v in packet['complete_old_table_anchors']}
for unused in NEW_ARCHIVES-used:(R/unused).unlink()
p='src/source_reflows_merge/main_e177_final_inputs.rs';before=G(HEAD,p);assert held(p)==before;s=before.decode();a=s.index('fn compiled(');z=s.index('fn read_view(',a);block='fn compiled(source: &str) -> Option<&\'static str> {\n    match source {\n'+''.join('        '+json.dumps(r['source'])+' => Some(include_str!("../../'+r['source']+'")),\n'for r in packet['rows']if r['compiled'])+'        _ => None,\n    }\n}\n';s=s[:a]+block+s[z:]
for old,new in [(f'assert_eq!(data.len(), {len(priorraw)});',f'assert_eq!(data.len(), {len(raw)});'),(H(priorraw),H(raw)),('assert_eq!(packet["rows"].as_array().unwrap().len(), 49);',f'assert_eq!(packet["rows"].as_array().unwrap().len(), {len(packet["rows"])});')]:assert s.count(old)==1;s=s.replace(old,new)
assert len(s.splitlines())<=1500;(R/p).write_text(s)
# CONFIG-only strict successor: unchanged old guard implementation and public recipe.
gp=ST+'/guard_payload.py';oldguard=G(HEAD,gp);assert held(gp)==oldguard;t=oldguard.decode();node=next(n.value for n in ast.parse(t).body if isinstance(n,ast.Assign)and isinstance(n.targets[0],ast.Name)and n.targets[0].id=='CONFIG');cfg=ast.literal_eval(node)
def pin(p):b=held(p);return dict(path=p,length=len(b),sha256=H(b))
cfg['packet']=pin(pp);cfg['rows']=len(packet['rows'])
for p in sorted(set(cfg['modules'])|set(newmods)):cfg['modules'][p]=pin(p)
for p,v in cfg['carriers'].items():v.update(pin(p))
table=cfg['table'];table.update(pin(table['path']));prior=held(table['prior']['archive']).decode().splitlines();order=held(table['path']).decode().splitlines();added=[x for x in order if x not in prior];assert[x for x in order if x not in added]==prior;table['insertions']=[dict(at=i,path=p)for i,p in enumerate(order)if p in added];table['rows']=len(order)
# Only existing named Python acquisitions plus new finite small sources. Large
# Rust-only subjects do not become new Python strict dependencies.
for source in sorted(set(newmods)|{r['source']for r in required}):
 if source not in cfg['closed_prior_sources']:assert len(held(source))<=262144;cfg['closed_prior_sources'].append(source)
lines=t.splitlines(True);a=sum(map(len,lines[:node.lineno-1]))+node.col_offset;z=sum(map(len,lines[:node.end_lineno-1]))+node.end_col_offset;newguard=(t[:a]+repr(cfg)+t[z:]).encode();ast.parse(newguard);assert len(newguard)<=65536;(R/gp).write_bytes(newguard)
for p in ['docs/evidence/accepted-strict-guard-generation-20261017/check-source.py','docs/evidence/accepted-final6-source-binding-20261017/check-source.py','docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py']:
 b=G(HEAD,p);assert held(p)==b;s=b.decode();assert s.count(H(oldguard))==1;s=s.replace(H(oldguard),H(newguard));prefix='guard_raw=held(guard_helper,'if'verify-or-refresh'in p else'helper_raw=bounded(helper,';old=prefix+str(len(oldguard));new=prefix+str(len(newguard));assert s.count(old)==1;s=s.replace(old,new);ast.parse(s);(R/p).write_text(s)
(R/REL/'source-seal.json').write_text(json.dumps(dict(status='SOURCE_ONLY_PENDING_CONTROLS',head=HEAD,rows=len(packet['rows']),packet_length=len(raw),packet_sha256=H(raw),guard_length=len(newguard),guard_sha256=H(newguard),new_modules=newmods,actual_required_modules=len(cfg['modules']),table_rows=len(order),complete_old49_retained=True,old54_454_tables_unchanged=True,closed_python_acquisitions=len(cfg['closed_prior_sources']),strict_cap_unchanged=262144),indent=2)+'\n');print(json.dumps({'rows':len(packet['rows']),'packet':len(raw),'modules':len(cfg['modules']),'table':len(order),'guard':len(newguard)}))
