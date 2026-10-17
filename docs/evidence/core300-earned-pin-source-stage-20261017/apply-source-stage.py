from pathlib import Path
import json,hashlib,subprocess,shutil,ast,difflib,os,stat
R=Path('/Volumes/Semaprax/worktrees/core300-earned13-consumer-coupling-20261017');B='6efb0d75020a3feeb6ea7ce0660f29a69c8b2661';CAP=Path('/Volumes/Semaprax/provenance/core300-6ef-actual-earned13-capsule-20261017');REL='docs/evidence/core300-earned-pin-source-stage-20261017';ST='docs/evidence/main-e177-final-source-binding-20261017';D=R/REL
H=lambda b:hashlib.sha256(b).hexdigest();G=lambda p:subprocess.check_output(['git','show',B+':'+p],cwd=R)
def held(p):
 p=R/p;fd=os.open(p,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC);key=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  st=os.fstat(fd);assert stat.S_ISREG(st.st_mode)and st.st_size<=9753094
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(st.st_size+1)
  assert len(b)==st.st_size and key(st)==key(os.fstat(fd))==key(p.lstat());return b
 finally:os.close(fd)
def archive(raw,kind):
 p=ST+'/'+kind+'/'+H(raw)+'.txt';q=R/p;q.parent.mkdir(parents=True,exist_ok=True)
 if q.exists():assert held(p)==raw
 else:q.write_bytes(raw)
 return {'length':len(raw),'sha256':H(raw),'archive':p}
def pin(p):b=held(p);return dict(path=p,length=len(b),sha256=H(b))
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=R,text=True).strip()==B
assert not subprocess.check_output(['git','status','--porcelain'],cwd=R).strip()
shutil.copytree(CAP/'candidate-repository',R,dirs_exist_ok=True)
subprocess.run(['git','apply','--check',str(CAP/'actual13-pin-source.patch')],cwd=R,check=True)
subprocess.run(['git','apply',str(CAP/'actual13-pin-source.patch')],cwd=R,check=True)
stage=json.loads(held(REL+'/earned-source-stage.json'));owners=[x['source']for x in stage['rows']];assert owners==sorted(owners)and len(owners)==3
for row in stage['rows']:
 assert G(row['source'])==held(row['prior']['archive'])
 assert held(row['source'])==held(row['current']['archive'])
# Preserve every whole old74 row/view. Only the actual workspace row changes;
# two new finite current channels cover the remaining captured pin owners.
p=ST+'/current-inputs.json';oldpacket=G(p);assert held(p)==oldpacket;v=json.loads(oldpacket);assert len(v['rows'])==74
changed=[];added=[]
for source in owners:
 b=held(source);old=G(source);rows=[x for x in v['rows']if x['source']==source];prior=archive(old,'views');prior=dict(name='whole6ef-pre-earned-pins',**prior)
 if rows:
  assert len(rows)==1;r=rows[0];assert (r['current']['length'],r['current']['sha256'])==(len(old),H(old));r['views'].append(prior);r['current']=archive(b,'current');changed.append(source)
 else:
  v['rows'].append(dict(source=source,compiled=True,current=archive(b,'current'),views=[prior]));added.append(source)
assert changed==['tests/workspace/semantic_graph.rs'] and added==['platform-tests/wasm-scalar-browser-v1/test-fixtures.mjs','src/workspace_graph/type_proof_tests.rs']
# Entire preceding74 table is a new independent physical+compiled anchor.
a=archive(oldpacket,'views');old74path=a['archive'];v['complete_old_table_anchors'].append(dict(path=old74path,**a))
v['current_stage']=dict(kind='actual-earned6ef-source-stage-not-consumer-qualified',producer_head=B,source_stage=REL+'/earned-source-stage.json',changed_sources=owners)
newpacket=(json.dumps(v,indent=2)+'\n').encode();assert len(newpacket)<=262144 and len(v['rows'])==76;(R/p).write_bytes(newpacket)
p='src/source_reflows_merge/main_e177_final_inputs.rs';s=G(p).decode();assert held(p)==s.encode();edits=[]
def edit(a,b,count=1):
 global s
 assert s.count(a)==count,(a,s.count(a));s=s.replace(a,b);edits.append(dict(before=a,after=b,count=count))
anchor='        _ => None,\n    }\n}\nfn compiled('
edit(anchor,'        '+json.dumps(old74path)+' => Some(include_str!("../../'+old74path+'")),\n'+anchor)
anchor='        _ => None,\n    }\n}\nfn read_view('
edit(anchor,''.join('        '+json.dumps(source)+' => Some(include_str!("../../'+source+'")),\n'for source in added)+anchor)
edit('assert_eq!(data.len(), '+str(len(oldpacket))+');','assert_eq!(data.len(), '+str(len(newpacket))+');')
edit('assert_eq!(digest(data.as_bytes()), "'+H(oldpacket)+'");','assert_eq!(digest(data.as_bytes()), "'+H(newpacket)+'");')
edit('assert_eq!(packet["rows"].as_array().unwrap().len(), 74);','assert_eq!(packet["rows"].as_array().unwrap().len(), 76);')
back=s
for e in reversed(edits):assert back.count(e['after'])==e['count'];back=back.replace(e['after'],e['before'])
assert back.encode()==G(p);(R/p).write_text(s)
# Same guard algorithm; exact packet/actual module pins and closed prior list only.
p=ST+'/guard_payload.py';oldguard=G(p);assert held(p)==oldguard;s=oldguard.decode();node=next(n.value for n in ast.parse(s).body if isinstance(n,ast.Assign)and isinstance(n.targets[0],ast.Name)and n.targets[0].id=='CONFIG');cfg=ast.literal_eval(node)
cfg['packet']=pin(ST+'/current-inputs.json');cfg['rows']=76
for name in cfg['modules']:cfg['modules'][name]=pin(name)
for name in added:
 if name not in cfg['closed_prior_sources']:cfg['closed_prior_sources'].append(name)
lines=s.splitlines(True);a=sum(map(len,lines[:node.lineno-1]))+node.col_offset;z=sum(map(len,lines[:node.end_lineno-1]))+node.end_col_offset
newguard=(s[:a]+repr(cfg)+s[z:]).encode();ast.parse(newguard);(R/p).write_bytes(newguard)
for p in ['docs/evidence/accepted-strict-guard-generation-20261017/check-source.py','docs/evidence/accepted-final6-source-binding-20261017/check-source.py','docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py']:
 s=G(p).decode();assert held(p)==s.encode();assert s.count(H(oldguard))==1;s=s.replace(H(oldguard),H(newguard));prefix='guard_raw=held(guard_helper,'if'verify-or-refresh'in p else'helper_raw=bounded(helper,';a=prefix+str(len(oldguard));b=prefix+str(len(newguard));assert s.count(a)==1;s=s.replace(a,b);ast.parse(s);(R/p).write_text(s)
# All old incoming table/reader and original source membership are unchanged.
unchanged=['docs/evidence/main99f-full-controller-acquisition-20261017/inputs.json','docs/evidence/main-e177-full-controller-acquisition-20261017/inputs.json','src/source_reflows_merge/main99f_inputs.rs','src/source_reflows_merge/main_e177_inputs.rs','src/source_reflows_merge/next_big_batch_inputs.rs']
for name in unchanged:assert held(name)==G(name)
F6=json.loads(G('docs/evidence/ordinary-plan-final-ingress-20261017/current-inputs.json'));assert all(held(x['path'])==G(x['path'])for x in F6)
m174=json.loads(G('docs/evidence/main09013-source-namespaces-v1/input-domains.json'));assert len(m174['physical'])==174 and all(held(x['path'])==G(x['path'])for x in m174['physical'])
tracked=subprocess.check_output(['git','diff','--name-only'],cwd=R,text=True).splitlines();inverse=[]
for name in tracked:
 old=G(name);new=held(name);bn=name.replace('/','__')+'.txt'
 for sub,raw in [('coupling-whole-before',old),('coupling-whole-after',new)]:q=D/sub/bn;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(raw)
 delta=list(difflib.unified_diff(old.decode().splitlines(True),new.decode().splitlines(True),fromfile='a/'+name,tofile='b/'+name));inv=list(difflib.unified_diff(new.decode().splitlines(True),old.decode().splitlines(True),fromfile='a/'+name,tofile='b/'+name))
 inverse.append(dict(source=name,before_length=len(old),before_sha256=H(old),after_length=len(new),after_sha256=H(new),before_archive=REL+'/coupling-whole-before/'+bn,after_archive=REL+'/coupling-whole-after/'+bn,forward=''.join(delta),inverse=''.join(inv)))
(D/'coupling-whole-inverses.json').write_text(json.dumps(inverse,indent=2)+'\n')
receipt=dict(status='SOURCE_ONLY_NOT_APPLIED_TO_ROOT_OR_CONSUMER_QUALIFIED',producer_head=B,consumer_source_head='pending coherent commit',source_owners=owners,packet_rows=76,packet_length=len(newpacket),packet_sha256=H(newpacket),guard_length=len(newguard),guard_sha256=H(newguard),required_current_modules=len(cfg['modules']),compiled_source_channels_added=added,whole_old74_anchor=old74path,immutable454_471_readers_and54_unchanged=unchanged,physical174_members_byteexact=True,FINAL6_six_byteexact=True,raw3_and_ordered451_unchanged=True,new_Rust_modules=0,production_algorithms_prices_caps_unchanged=True,strict_small_cap=262144,strict_controls='pending source-only execution',actual_sourcepin_provenance=B,feature953_outputs_not_claimed=True)
(D/'coupling-receipt.json').write_text(json.dumps(receipt,indent=2)+'\n');print(json.dumps(receipt,indent=2))
