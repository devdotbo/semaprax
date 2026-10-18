from pathlib import Path
import json,hashlib,copy,subprocess
R=Path('/Volumes/Semaprax/worktrees/next-large-combined-20261017');D=Path(__file__).parent;E='docs/evidence/v010-5075-four-source-reseal-20261018';P='docs/evidence/finite-source-overlay-v2-20261018/';H=lambda b:hashlib.sha256(b).hexdigest();payload=R/'docs/evidence/main-e177-final-source-binding-20261017/guard_payload.py';raw=payload.read_bytes();ns={'__file__':str(payload),'_AUTHENTICATED_PROGRAM':raw};exec(compile(raw,str(payload),'exec'),ns);logical=ns['packet']();oldpage=json.loads((R/E/'before'/(P.replace('/','__')+'page.json.txt')).read_bytes());oldpolicy=json.loads((R/E/'before'/(P.replace('/','__')+'policy.json.txt')).read_bytes());old=ns['_overlay']().predecessor();kernel={};exec(compile((R/P/'overlay_page.py').read_bytes(),P+'overlay_page.py','exec'),kernel);oldlogical=kernel['compose_values'](old,oldpolicy,oldpage);assert len(oldlogical['rows'])==len(logical['rows'])==543
for before,after in zip(oldlogical['rows'],logical['rows']):
 assert before['source']==after['source']and before['compiled']==after['compiled']and after['views'][:len(before['views'])]==before['views'];restored=copy.deepcopy(after);restored['current']=before['current'];restored['views']=before['views']
 if'fragments'in before:
  assert [c['source']for c in before['fragments']]==[c['source']for c in after['fragments']];restored['fragments']=before['fragments']
 assert restored==before
channels=0;cache={}
def read(p):
 if p not in cache:cache[p]=(R/p).read_bytes()
 return cache[p]
for row in logical['rows']:
 b=read(row['source']);pin=row['current'];assert(len(b),H(b))==(pin['length'],pin['sha256']),row['source']
 for pin in [row['current'],*row['views']]:b=read(pin['archive']);assert(len(b),H(b))==(pin['length'],pin['sha256']);channels+=1
for root in ns['CONFIG']['carriers']:ns['authenticate_current'](root,read(root).decode())
edges=0
for name in ['hir','projection']:
 model=json.loads(read('docs/evidence/a3-e47-final-custody-20261016/'+name+'.json'))
 for p,pin in model['sources'].items():b=read(p);assert(len(b),H(b))==(pin['length'],pin['sha256']),p
 for value in model['classes'].values():
  for chain in value['chains']:
   for e in chain['edges']:b=read(e['parent']);text=e['text'].encode();assert b[e['start']:e['start']+len(text)]==text;edges+=1
policy=json.loads(read(P+'policy.json'));receipt=json.loads(read(E+'/reseal-receipt.json'));result=dict(status='SOURCE_PIN_RESEAL_CHECK_ONLY',head=subprocess.check_output(['git','rev-parse','HEAD'],cwd=R).decode().strip(),rows=543,whole_prior543_per_row_inverse=True,all_prior_view_prefixes=True,channels=channels,modeled_edges=edges,raw_modules=len(ns['CONFIG']['modules']),closed_sources=len(ns['CONFIG']['closed_prior_sources']),owners=342,ordered_rows=ns['CONFIG']['table']['rows'],page=policy['page'],raw_changes=receipt['changed_sources'],model_source_pin_changes=len(receipt['model_source_changes']),typed_edge_offset_changes=len(receipt['typed_edge_offset_changes']),walk_lines=len(read('src/cleanup_plan/replay/path_join/selected/walk.rs').splitlines()),root_lines=len(read('src/source_reflows_merge/main_e177_final_inputs.rs').splitlines()),no_source_producer_or_Rust_test_run=True,Cargo=False)
for p,b in cache.items():assert(R/p).read_bytes()==b
(R/E/'current-pin-proof.json').write_text(json.dumps(result,indent=2)+'\n');print(json.dumps(result,indent=2))
