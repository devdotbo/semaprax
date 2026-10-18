from pathlib import Path
import subprocess,json,hashlib,os,stat,copy
R=Path('/Volumes/Semaprax/worktrees/focused146-structural-repairs-20261018');D=Path(__file__).parent;E='docs/evidence/finite-source-overlay-v2-20261018';H=lambda b:hashlib.sha256(b).hexdigest();head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R).decode().strip();assert not subprocess.check_output(['git','status','--porcelain'],cwd=R).strip();cache={}
def held(p,cap=9753094):
 if p in cache:return cache[p]
 q=R/p;assert q.is_relative_to(R)and all(not x.is_symlink()for x in [q,*list(q.parents)[:len(q.parts)-len(R.parts)]])
 fd=os.open(q,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 try:
  st=os.fstat(fd);assert stat.S_ISREG(st.st_mode)and 0<=st.st_size<=cap
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(st.st_size);assert len(b)==st.st_size and not f.read(1)
  key=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
  assert key(st)==key(os.fstat(fd))==key(os.lstat(q));cache[p]=b;return b
 finally:os.close(fd)
payload='docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py';raw=held(payload);ns={'__file__':str(R/payload),'_AUTHENTICATED_PROGRAM':raw};exec(compile(raw,str(R/payload),'exec'),ns);logical=ns['packet']();cfg=ns['CONFIG'];policy=json.loads(held(E+'/policy.json'));page=json.loads(held(E+'/page.json'));base=json.loads(Path('/Volumes/Semaprax/provenance/focused146-overlay-binding-scope-20261018/whole519-b5a7-canonical.json').read_bytes());inverse=copy.deepcopy(logical);inverse['rows']=base['rows'];assert inverse==base
paths={row['source']for row in logical['rows']}|set(cfg['modules'])|{payload,str(ns['_CONFIG_PATH'].relative_to(R)),E+'/policy.json',E+'/page.json',E+'/overlay_page.py','src/source_reflows_merge/main_e177_final_inputs.rs'}
models={name:json.loads(held('docs/evidence/a3-e47-final-custody-20261016/'+name+'.json',16777216))for name in ['hir','projection']}
for model in models.values():paths.update(model['sources'])
# Immutable Git objects are acquired in one batch, with exact held live equality.
ordered=sorted(paths);proc=subprocess.run(['git','cat-file','--batch'],cwd=R,input=''.join(head+':'+p+'\n'for p in ordered).encode(),stdout=subprocess.PIPE,check=True);data=proc.stdout;offset=0
for p in ordered:
 end=data.index(b'\n',offset);header=data[offset:end].split();assert len(header)==3 and header[1]==b'blob',(p,header);size=int(header[2]);offset=end+1;blob=data[offset:offset+size];assert len(blob)==size and data[offset+size:offset+size+1]==b'\n';offset+=size+1;assert held(p,16777216)==blob,p
assert offset==len(data)
channels=0;archives=set()
for row in logical['rows']:
 b=held(row['source']);pin=row['current'];assert(len(b),H(b))==(pin['length'],pin['sha256']),row['source']
 for pin in [row['current'],*row['views']]:
  b=held(pin['archive']);assert(len(b),H(b))==(pin['length'],pin['sha256']);channels+=1;archives.add(pin['archive'])
for p,pin in cfg['modules'].items():b=held(p);assert(len(b),H(b))==(pin['length'],pin['sha256'])
edges=0
for name,model in models.items():
 for p,pin in model['sources'].items():b=held(p);assert(len(b),H(b))==(pin['length'],pin['sha256']),(name,p)
 for value in model['classes'].values():
  for chain in value['chains']:
   for edge in chain['edges']:
    text=edge['text'].encode();b=held(edge['parent']);assert b[edge['start']:edge['start']+len(text)]==text,(name,edge['parent'],edge['child']);edges+=1
oldedges=ns['_overlay']().predecessor()['rows'];oldedgecount=sum(len(r.get('fragments',[]))for r in oldedges);newedgecount=sum(len(r.get('fragments',[]))for r in logical['rows']);assert oldedgecount==10 and newedgecount-oldedgecount==len(policy['new_fragment_edges'])==66
for row in logical['rows']:
 for child in row.get('fragments',[]):assert not child.get('fragments')and child==next(r for r in logical['rows']if r['source']==child['source'])and child['compiled']is True
stage_count=0;affected=set(policy['replacements'])|{x['source']for x in policy['additions']};by={r['source']:r for r in logical['rows']}
for row in ns['_successor_rows']():
 if row['source']not in affected:continue
 choices={(p['length'],p['sha256'])for p in [by[row['source']]['current'],*by[row['source']]['views']]}
 for pin in [row['before'],row['current'],*row.get('views',[])]:assert(pin['length'],pin['sha256'])in choices;stage_count+=1
for source in ['src/source_reflows/incoming090_semantic_receivers.rs','src/source_reflows_merge/ordinary_plan_inputs.rs']:
 row=by[source]
 for pin in [row['current'],*row['views']]:assert ns['latest_source'](source,pin['length'],pin['sha256'])==held(pin['archive'])
for root in cfg['carriers']:ns['authenticate_current'](root,held(root).decode())
role=json.loads((D/'actual-role-visibility/receipt.json').read_bytes());assert role['current_owners']==role['old_owners']+role['actual_new_classified_modules']and not role['role_reclassifications'];assert all(role['negative_controls'].values())
proof=dict(status='SOURCE_ONLY_FINAL_BOUND_OVERLAY_AND_ROLES',head=head,rows=len(logical['rows']),prior_rows=519,replacements=len(page['replacements']),additions=len(page['additions']),channels=channels,unique_archives=len(archives),old_fragment_edges=oldedgecount,new_audit_edges=newedgecount-oldedgecount,original_stage_tuples_preserved=stage_count,whole519_object_inverse=True,all_current_physical_equals_Git_and_archive=True,all_current_model_sources_equal_Git=True,all_current_model_edges_actual=edges,Git_batch_paths=len(paths),raw_modules=len(cfg['modules']),old_raw_module_prefix_preserved=166,closed_prior_sources=len(cfg['closed_prior_sources']),old_closed_ordered_prefix_preserved=564,ordered_rows=cfg['table']['rows'],old_ordered_subsequence=1010,owners=role['current_owners'],old_owners=role['old_owners'],actual_new_modules=role['actual_new_classified_modules'],role_reclassifications=[],role_negative_controls=role['negative_controls'],page=policy['page'],policy=dict(length=len(held(E+'/policy.json')),sha256=H(held(E+'/policy.json'))),control_child=dict(length=len(held('src/source_reflows_merge/main_e177_overlay_v2.rs')),sha256=H(held('src/source_reflows_merge/main_e177_overlay_v2.rs')),lines=len(held('src/source_reflows_merge/main_e177_overlay_v2.rs').splitlines())),root_lines=len(held('src/source_reflows_merge/main_e177_final_inputs.rs').splitlines()),actual_source_producer_heads=dict(subjects174='8ff442ae01deafae1587b7824ab804eb73353169',sources6='5133f4b606d1a4e39a74e2880c3222dd8cc2840f'),source_producer_status_scope='source-only; NOT executed174/6Rusttests; final current outputs and predecessor channels independently authenticated',Rust_tests_executed=False,Cargo=False)
assert subprocess.check_output(['git','rev-parse','HEAD'],cwd=R).decode().strip()==head
for p,b in cache.items():assert(R/p).read_bytes()==b,p
(D/'final-source-proof.json').write_text(json.dumps(proof,indent=2)+'\n');print(json.dumps(proof,indent=2))
