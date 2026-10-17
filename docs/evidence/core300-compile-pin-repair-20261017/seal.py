from pathlib import Path
import json,hashlib,subprocess,ast,os,stat
R=Path('/Volumes/Semaprax/worktrees/wasm-custody-compile-repair-20261017');HEAD='675247f9bef668e0c41378409e17c2a126c68475';BASE='a6c92a197a21ed3e8c67c842b20886f2240cb0e2';REL='docs/evidence/core300-compile-pin-repair-20261017';D=R/REL;D.mkdir(exist_ok=True);ST='docs/evidence/main-e177-final-source-binding-20261017';H=lambda b:hashlib.sha256(b).hexdigest();G=lambda p:subprocess.check_output(['git','show',BASE+':'+p],cwd=R)
def held(p,cap=9753094):
 fd=os.open(R/p,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC);I=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  st=os.fstat(fd);assert stat.S_ISREG(st.st_mode)and st.st_size<=cap
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(st.st_size);assert len(b)==st.st_size and not f.read(1)
  assert I(st)==I(os.fstat(fd))==I(os.lstat(R/p));return b
 finally:os.close(fd)
def archive(raw,sub):
 p=ST+'/'+sub+'/'+H(raw)+'.txt';q=R/p;q.parent.mkdir(parents=True,exist_ok=True)
 if q.exists():assert held(p)==raw
 else:q.write_bytes(raw)
 return dict(length=len(raw),sha256=H(raw),archive=p)
def pin(p):b=held(p);return dict(path=p,length=len(b),sha256=H(b))
# Exactly one changed incoming471 current tuple; every incoming field remains whole.
p='docs/evidence/main-e177-full-controller-acquisition-20261017/inputs.json';old471=G(p);assert held(p)==old471;v=json.loads(old471);updates=[]
for r in v['rows']:
 b=held(r['path']);pair=(len(b),H(b))
 if pair!=(r['current_length'],r['current_sha256']):updates.append(r['path']);r['current_length'],r['current_sha256']=pair
assert updates==['src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs'],updates
new471=(json.dumps(v,indent=2)+'\n').encode();assert len(new471)<=262144;(R/p).write_bytes(new471)
p='src/source_reflows_merge/main_e177_inputs.rs';s=G(p).decode();assert held(p)==s.encode()
for a,z in [(f'assert_eq!(data.len(), {len(old471)}); // MAIN_E177_PACKET_LENGTH',f'assert_eq!(data.len(), {len(new471)}); // MAIN_E177_PACKET_LENGTH'),(H(old471),H(new471))]:
 if a!=z:assert s.count(a)==1;s=s.replace(a,z)
(R/p).write_text(s)
# Seven-path Main6d successor: only the actually changed source tuple and stage label.
p='docs/evidence/main6d-closed-controller-acquisition-20261017/inputs.json';old7=G(p);assert held(p)==old7;v=json.loads(old7);changes=[]
for r in v['rows']:
 b=held(r['source']);c=r['current']
 if (len(b),H(b))!=(c['length'],c['sha256']):
  changes.append(r['source']);a='docs/evidence/main6d-closed-controller-acquisition-20261017/current/'+H(b)+'.txt';(R/a).write_bytes(b);r['current']=dict(path=a,length=len(b),sha256=H(b))
assert changes==updates;v['current_source_stage']=dict(kind='reviewed-working-stage',git_head=HEAD,changed_sources=changes);new7=(json.dumps(v,indent=2)+'\n').encode();assert len(new7)<=262144;(R/p).write_bytes(new7)
p='src/source_reflows/semantic_receivers/status_inline.rs';s=G(p).decode();assert held(p)==s.encode()
for a,z,count in [(f'assert_eq!(data.len(), {len(old7)});',f'assert_eq!(data.len(), {len(new7)});',1),('inputs.json"),'+str(len(old7))+',"','inputs.json"),'+str(len(new7))+',"',1),(H(old7),H(new7),2)]:
 if a!=z:assert s.count(a)==count;s=s.replace(a,z)
(R/p).write_text(s)
# Retain every whole old73 row/view; add only the newly changed old54 source.
p=ST+'/current-inputs.json';old73=G(p);assert held(p)==old73;v=json.loads(old73);assert len(v['rows'])==73;changed=[]
for r in v['rows']:
 b=held(r['source']);old=r['current']
 if (len(b),H(b))!=(old['length'],old['sha256']):
  changed.append(r['source']);seen={(x['length'],x['sha256'])for x in r['views']}
  if (old['length'],old['sha256'])not in seen:r['views'].append(dict(name='whole-a6c73-current',**old))
  r['current']=archive(b,'current')
source='src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs';assert all(r['source']!=source for r in v['rows']);old54=json.loads(held('docs/evidence/next-big-batch-source-binding-20261017/current-inputs.json'));prior=next(r for r in old54['rows']if r['source']==source)['current'];b=held(prior['archive']);assert(len(b),H(b))==(prior['length'],prior['sha256']);assert b==G(source)
v['rows'].append(dict(source=source,compiled=True,current=archive(held(source),'current'),views=[dict(name='whole-afa54-current-and-a6c-immediate',**prior)]));v['current_stage']=dict(kind='reviewed-working-stage',git_head=HEAD,changed_sources=changed+[source]);new74=(json.dumps(v,indent=2)+'\n').encode();assert len(new74)<=262144;(R/p).write_bytes(new74)
p='src/source_reflows_merge/main_e177_final_inputs.rs';s=G(p).decode();assert held(p)==s.encode();a='        _ => None,\n    }\n}\nfn read_view(';z='        '+json.dumps(source)+' => Some(include_str!("../../'+source+'")),\n'+a;assert s.count(a)==1;s=s.replace(a,z)
for a,z in [(f'assert_eq!(data.len(), {len(old73)});',f'assert_eq!(data.len(), {len(new74)});'),(H(old73),H(new74)),('assert_eq!(packet["rows"].as_array().unwrap().len(), 73);','assert_eq!(packet["rows"].as_array().unwrap().len(), 74);')]:assert s.count(a)==1;s=s.replace(a,z)
(R/p).write_text(s)
p=ST+'/guard_payload.py';oldguard=G(p);assert held(p)==oldguard;s=oldguard.decode();node=next(n.value for n in ast.parse(s).body if isinstance(n,ast.Assign)and isinstance(n.targets[0],ast.Name)and n.targets[0].id=='CONFIG');cfg=ast.literal_eval(node);cfg['packet']=pin(ST+'/current-inputs.json');cfg['rows']=74
for name in cfg['modules']:cfg['modules'][name]=pin(name)
assert source not in cfg['closed_prior_sources'];cfg['closed_prior_sources'].append(source)
lines=s.splitlines(True);a=sum(map(len,lines[:node.lineno-1]))+node.col_offset;z=sum(map(len,lines[:node.end_lineno-1]))+node.end_col_offset;newguard=(s[:a]+repr(cfg)+s[z:]).encode();ast.parse(newguard);(R/p).write_bytes(newguard)
for p in ['docs/evidence/accepted-strict-guard-generation-20261017/check-source.py','docs/evidence/accepted-final6-source-binding-20261017/check-source.py','docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py']:
 s=G(p).decode();assert held(p)==s.encode();assert s.count(H(oldguard))==1;s=s.replace(H(oldguard),H(newguard));prefix='guard_raw=held(guard_helper,'if'verify-or-refresh'in p else'helper_raw=bounded(helper,';a=prefix+str(len(oldguard));z=prefix+str(len(newguard));assert s.count(a)==1;s=s.replace(a,z);ast.parse(s);(R/p).write_text(s)
# All six original FINAL6 source subjects are byte-identical; retain binder output.
six=['src/cleanup_plan/replay.rs','src/source_reflows_merge.rs','src/source_reflows_merge/main67107_inputs.rs','tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs','crates/semaprax-native-rust-interop-builder/src/implementation/tests/ledger_capacity/source_inventory.rs'];assert all(held(p)==G(p)for p in six)
(D/'source-seal.json').write_text(json.dumps(dict(status='SOURCE_ONLY_PENDING_STRICT_CONTROLS',base=BASE,source_head=HEAD,rows=74,packet_length=len(new74),packet_sha256=H(new74),guard_length=len(newguard),guard_sha256=H(newguard),updated73=changed,new_row=source,changed471=updates,changed7=changes,FINAL6_six_unchanged=six,raw3_and_ordered_table_unchanged=True,required_module_count=len(cfg['modules']),strict_cap_unchanged=262144),indent=2)+'\n');print(len(new74),len(newguard),len(changed))
