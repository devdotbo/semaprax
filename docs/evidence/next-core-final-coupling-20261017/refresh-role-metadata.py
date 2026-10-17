"""Finite existing220-owner registration delta; no full root enumeration or compiler."""
from pathlib import Path
import argparse,ast,json,re,hashlib,copy,difflib,subprocess,importlib.util,os,stat
BASE='0f9fb3766f0413694e4a12447538f7c7bb99627f';SOURCE=BASE;REL='docs/evidence/next-combined-role-metadata-20261017';F='docs/evidence/a3-e47-final-custody-20261016/';B='docs/evidence/source-boundary-domains-20261016/'
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);p.add_argument('--expected-head',required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--write',action='store_true');a=p.parse_args();R=a.repo.resolve();assert R==Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang');git=lambda *x:subprocess.check_output(['git',*x],cwd=R);assert re.fullmatch('[0-9a-f]{40}',a.expected_head)and git('rev-parse','HEAD').decode().strip()==a.expected_head
assert not git('status','--porcelain','--untracked-files=all').strip(),'metadata starts at clean post174/postFINAL6 commit';assert not a.out.exists();a.out.mkdir(parents=True)
H=lambda b:hashlib.sha256(b).hexdigest();enc=lambda v:(json.dumps(v,indent=2)+'\n').encode();G=lambda rev,p:git('show',rev+':'+p)
SNAPSHOTS={}
def held(path,cap=1048576):
 fd=os.open(R/path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC);ident=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  st=os.fstat(fd);assert stat.S_ISREG(st.st_mode)and st.st_size<=cap
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(st.st_size);assert len(b)==st.st_size and not f.read(1)
  assert ident(st)==ident(os.fstat(fd))==ident(os.lstat(R/path))
  if path in SNAPSHOTS:assert SNAPSHOTS[path]==b,('source changed during finite metadata preparation',path)
  else:SNAPSHOTS[path]=b
  return b
 finally:os.close(fd)
def sealed(path,n,h):b=held(path);assert(len(b),H(b))==(n,h);return b
parserpath='docs/evidence/production-source-authority-partition-20261016/original-module-parser-prefix.py';parser=held(parserpath);assert H(parser)=='87a903ce0f9927d3d90063fdd686b50b5302e2ac10cfea324918f7c030b28599'
genpath=F+'generate-authority.py';gen=held(genpath);assert H(gen)=='0c4b31857cdb8a4f645d61c6f0d2453c33c6adddb8c6f26d9c37b56620841590'
toolpath=B+'boundary_tools.py';tool=held(toolpath);assert tool==G(SOURCE,toolpath)
NEW={'src/cleanup_plan/replay/factored/canonical_shared/before_selected_route.rs', 'src/cleanup_plan/replay/factored/canonical_shared/before_scope_memo.rs', 'src/cleanup_plan/replay/factored/canonical_shared/empty_memo/owned_tests.rs', 'src/source_reflows/semantic_receivers/status_inline.rs', 'src/cleanup_plan/replay/factored/canonical_shared/paid_route_tests.rs', 'src/cleanup_plan/replay/factored/canonical_shared/before_paid_many/empty_memo.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained/inline_status.rs', 'src/cleanup_plan/replay/expression_facts/receiving/retained/inline_status/tests.rs', 'src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/inline.rs', 'src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix/inline/tests.rs'} # Exact ten-module CORE union, excluding experimental883.
ADDED_RS={x for x in git('diff','--name-only','--diff-filter=A',BASE,a.expected_head).decode().splitlines()if x.endswith('.rs')and x.startswith(('src/','tests/','crates/'))};assert ADDED_RS==NEW,('new physical module requires an explicit reviewed delta',sorted(ADDED_RS))
ALLOWED_CHANGED=set(git('diff','--name-only',BASE,a.expected_head).decode().splitlines())
RECLASSIFY={'src/cleanup_plan/replay/factored/canonical_shared/selected_labels.rs'}
REACH={'src/source_reflows_merge/main_e177_final_inputs.rs','src/source_reflows_merge/next_big_batch_inputs.rs'}|RECLASSIFY
before={};full={}
for name in ['hir','projection']:
 path=F+name+'.json';b=held(path,16777216);assert b==G(SOURCE,path);before[path]=b;full[name]=json.loads(b)
# Authenticated old metadata source domain is whole9ad, regardless of newHEAD label.
oldblobs={};now={}
for m in full.values():
 for path,row in m['sources'].items():
  if path not in oldblobs:oldblobs[path]=G(BASE,path)
  b=oldblobs[path];assert(len(b),H(b))==(row['length'],row['sha256']),(path,'old source metadata not exact9ad')
  if path not in now:now[path]=held(path,9753094)
for path in NEW:held(path,9753094)
ns={};exec(compile(parser,parserpath,'exec'),ns);ns.update(R=R,seen=set(),snapshots=[],IDX={});original=ns['parse'];captured={}
PARENTS={path for path in oldblobs if path.endswith('.rs') and oldblobs[path]!=now[path] and re.search(r'\bmod\s+(?:r#)?\w+\s*[;{]',ns['mask'](now[path].decode()))}
# Reparse the exact narrowed selected subtree from its real current canonical parent.
RECLASS_SUBTREE={path for model in full.values()for path,value in model['classes'].items()if any(any(e['child']in RECLASSIFY for e in c['edges'])for c in value['chains'])}
PARENTS-=RECLASS_SUBTREE
# One exact unchanged registered ancestor reaches the new test child through
# an existing test-parent file absent from the old raw scope. Parse only this
# real chain, with original cfg/path semantics; do not mint a manual role.
BRIDGES=set() # The previously missing source-test chain is already in baseline220.
assert {'tests/wasm.rs','tests/workspace.rs'}<=PARENTS
for bridge in BRIDGES:
 b=held(bridge,9753094);assert b==G(a.expected_head,bridge),'whole current registered bridge drift'
def capture(path,prefix,chain,kind,package,custom_path=False):
 rel=str(Path(path).resolve().relative_to(R));undernew=any(n['source_path']in NEW|RECLASSIFY for n in chain)
 if rel not in NEW|PARENTS|BRIDGES|REACH and not undernew:return
 captured.setdefault(rel,[]).append((kind,chain,package,None));return original(path,prefix,chain,kind,package,custom_path)
ns['parse']=capture
transforms={}
def remap(path,at,text):
 old=oldblobs[path];current=now[path];assert old[at:at+len(text)]==text
 if path not in transforms:
  left=old.splitlines(True);right=current.splitlines(True);lo=[0];ro=[0]
  for x in left:lo.append(lo[-1]+len(x))
  for x in right:ro.append(ro[-1]+len(x))
  transforms[path]=[(lo[i],lo[i+n],ro[j]-lo[i])for i,j,n in difflib.SequenceMatcher(a=left,b=right,autojunk=False).get_matching_blocks()if n]
 found=[at+d for x,y,d in transforms[path]if x<=at and at+len(text)<=y]
 if len(found)!=1:found=[x.start()for x in re.finditer(re.escape(text),current)]
 assert len(found)==1,(path,text);assert current[found[0]:found[0]+len(text)]==text;return found[0]
for m in full.values():
 for cls in m['classes'].values():
  for c in cls['chains']:
   if any(e['child']in RECLASSIFY for e in c['edges']):continue # Replaced from the actual cfg(test) registration below.
   for e in c['edges']:e['start']=remap(e['parent'],e['start'],e['text'].encode())
for parent in sorted(PARENTS):
 chains=[]
 for m in full.values():
  for cls in m['classes'].values():
   for c in cls['chains']:
    if c['root']==parent:
     v=dict(c);v['edges']=[]
     if v not in chains:chains.append(v)
    for i,e in enumerate(c['edges']):
     if e['child']!=parent:continue
     v=dict(c);v['edges']=c['edges'][:i+1]
     if v not in chains:chains.append(v)
 assert chains,('registered parent witness missing',parent)
 for c in chains:
  nodes=[]
  for e in c['edges']:
   md=re.search(r'\bmod\s+(\w+)\s*[;{]',e['text']);assert md
   nodes.append(dict(source_path=e['parent'],line=held(e['parent'],9753094)[:e['start']].count(b'\n')+1,module=md[1],attributes=e['attributes']))
  kind='lib'if c['kind']=='lib'else('bin:semaprax'if c['kind']=='bin'else'test:'+Path(c['root']).stem);custom=bool(nodes and any(re.search(r'path\s*=',attr)for attr in nodes[-1]['attributes']));capture(R/parent,[x['module']for x in nodes],nodes,kind,c['package'],custom_path=custom)
assert NEW|REACH<=set(captured),('unregistered required children',sorted((NEW|REACH)-set(captured)))
def klass(path):
 chains=[];closed=[]
 for kind,nodes,package,_ in captured[path]:
  edges=[]
  for i,node in enumerate(nodes):
   text=held(node['source_path'],9753094).decode();masked=ns['mask'](text);startline=sum(len(x)for x in text.splitlines(True)[:node['line']-1]);match=re.search(r'(?:pub(?:\([^)]*\))?\s+)?mod\s+'+re.escape(node['module'])+r'\s*[;{]',masked[startline:]);assert match;start=startline+match.start();end=startline+match.end()
   for attr in reversed(node['attributes']):start=text.rfind(attr,0,start);assert start>=0
   edges.append(dict(parent=node['source_path'],start=len(text[:start].encode()),text=text[start:end],attributes=node['attributes'],child=nodes[i+1]['source_path']if i+1<len(nodes)else path))
  roots={('semaprax','lib'):'src/lib.rs',('semaprax','bin:semaprax'):'src/main.rs'};root=roots.get((package,kind),'tests/'+kind.removeprefix('test:')+'.rs');v=dict(root=root,kind='lib'if kind=='lib'else'bin'if kind.startswith('bin:')else'test',package=package,edges=edges)
  if v not in chains:chains.append(v)
  closed.append(kind.startswith('test:')or any('#[cfg(test)]'in n['attributes']for n in nodes))
 return dict(role='test-only'if all(closed)else'production',chains=chains)
# Require actual unchanged helper items to be reachable from the new independent roots.
for source,roots in {'src/source_reflows_merge/main_e177_final_inputs.rs':{'tests/wasm.rs','tests/workspace.rs'},'src/source_reflows_merge/next_big_batch_inputs.rs':{'tests/workspace.rs'}}.items():
 assert roots<={c['root']for c in klass(source)['chains']},(source,'missing real harness registration')
# Only the two already-owned literal inventory expressions are parsed.
text=gen.decode();begin=text.index("mask=ns['mask']");end=text.index('needed={x[\'path\']');fragment=text[begin:end]
JOIN='macro_rules! audit_source_join { ($($piece:expr),* $(,)?) => {{ let pieces: &[&str] = &[$(&$piece),*]; pieces.concat() }}; }'
grammar_views={};grammar_receipts=[]
for carrier in ['tests/hir_module_boundaries.rs','tests/workspace/semantic_graph/source_inventory.rs']:
 physical=held(carrier,262144);assert physical==G(a.expected_head,carrier)
 raw=physical.decode();assert raw.count(JOIN)==1;at=raw.index(JOIN)
 view=JOIN+raw[:at]+raw[at+len(JOIN):]
 assert view[len(JOIN):len(JOIN)+at]+JOIN+view[len(JOIN)+at:]==raw
 grammar_views[carrier]=view;grammar_receipts.append(dict(source=carrier,physical_length=len(physical),physical_sha256=H(physical),join_original_offset=at,grammar_view_sha256=H(view.encode()),complete_inverse_verified=True))
for carrier in grammar_views:
 original="(R/"+repr(carrier)+").read_text()";replacement="_literal_inventory_source("+repr(carrier)+")"
 assert fragment.count(original)==1;fragment=fragment.replace(original,replacement,1)
ex={'ns':ns,'R':R,'re':re,'json':json,'_literal_inventory_source':lambda path:grammar_views[path]};exec(compile(fragment,genpath,'exec'),ex)
for name,m in full.items():
 old=json.loads(before[F+name+'.json']);chunks=ex[name+'_chunks'];oldpaths={x['path']for x in old['chunks']if'path'in x};newchunks=[x for x in chunks if 'path'in x and x['path']not in oldpaths];assert all('path'in x and x['path']in NEW for x in newchunks)
 assert[x for x in chunks if x not in newchunks]==old['chunks'],(name,'old ordered chunks changed')
 m['chunks']=chunks;rawpaths={x['path']for x in chunks if'path'in x};included={p for p,records in captured.items()if p in rawpaths or any(any(n['source_path']in rawpaths for n in row[1])for row in records)}
 affected={path for path,value in old['classes'].items()if any(any(e['child']in RECLASSIFY for e in chain['edges'])for chain in value['chains'])}
 for path in affected:
  m['classes'][path]['chains']=[chain for chain in m['classes'][path]['chains']if not any(e['child']in RECLASSIFY for e in chain['edges'])]
 for path in sorted(included):
  value=klass(path)
  if path in m['classes']:
   assert value['role']==m['classes'][path]['role']or value['role']=='test-only'
   for chain in value['chains']:
    if chain not in m['classes'][path]['chains']:m['classes'][path]['chains'].append(chain)
  else:m['classes'][path]=value
 assert set(old['classes'])<=set(m['classes'])and set(m['classes'])-set(old['classes'])<=NEW
 for path in affected:
  chains=m['classes'][path]['chains'];assert chains,('lost existing selected subtree registration',path)
  closed=[c['kind']=='test'or any('#[cfg(test)]'in e['attributes']for e in c['edges'])for c in chains]
  current_role='test-only'if all(closed)else'production';assert current_role=='test-only',('selected subtree remained shipping',path)
  m['classes'][path]['role']=current_role
 for path,value in old['classes'].items():
  assert m['classes'][path]['role']==value['role']or(path in affected and value['role']=='production'and m['classes'][path]['role']=='test-only')
 m['closure_only']=sorted(set(m['classes'])-rawpaths);paths=set(m['sources'])|included
 for cls in m['classes'].values():
  for c in cls['chains']:
   paths.add(c['root'])
   for e in c['edges']:paths.update([e['parent'],e['child']]);b=held(e['parent'],9753094);assert b[e['start']:e['start']+len(e['text'].encode())]==e['text'].encode()
 m['sources']={p:dict(length=len(held(p,9753094)),sha256=H(held(p,9753094)))for p in sorted(paths)}
 # Old class/chunk roles remain exact; all new actual children have compiled paths.
 output=a.out/(name+'.json');output.write_text(json.dumps(m,separators=(',',':'))+'\n')
ownpath=F+'new-owners.json';raw=held(ownpath);assert raw==G(SOURCE,ownpath);before[ownpath]=raw;owners=json.loads(raw);assert len(owners)==220;have={r['source']for r in owners}
for path in sorted(NEW-have):owners.append(dict(source=path))
assert len(owners)==220+len(NEW-have)
for row in owners:
 b=held(row['source'],9753094)
 if row['source']not in ALLOWED_CHANGED:assert(row['sha256'],row['lines'])==(H(b),len(b.splitlines())),('unchanged old owner drift',row['source'])
 row.update(sha256=H(b),lines=len(b.splitlines()))
(a.out/'new-owners.json').write_bytes(enc(owners))
# Retain the exact helper before executing it; never relabel a full generator run.
ns_tool={'__file__':str(R/toolpath)};exec(compile(tool,toolpath,'exec'),ns_tool)
full_dir=a.out/'full';full_dir.mkdir()
for name in full:(full_dir/(name+'.json')).write_bytes((a.out/(name+'.json')).read_bytes())
prefix=text.split("for name,chunks in [('hir',hir_chunks),('projection',projection_chunks)]:",1)[0];manifest,proof=ns_tool['generated_scope'](R,full_dir,regenerated=(full,H(prefix.encode())));proof['negative_controls']=ns_tool['negative_checks'](R,full_dir,manifest);proof['status']='SOURCE_ONLY_EXACT_BOUNDARIES_AND_COMPLETE_SOURCE_ROLE_CUSTODY_PASS';proof['registration_update']='FINITE_220_OLD_EDGE_REMAP_AND_EXISTING_HELPER_HARNESS_REGISTRATIONS';proof['global_polluted_purity_result_not_claimed']=True
for p in [B+'current-domains.json',B+'boundary-source-proof.json']:before[p]=held(p,16777216)
outputs={F+name+'.json':(a.out/(name+'.json')).read_bytes()for name in full};outputs[ownpath]=enc(owners);outputs[B+'current-domains.json']=enc(manifest);outputs[B+'boundary-source-proof.json']=enc(proof)
historical=F+'authority-source-proof.json';historic_raw=held(historical);assert historic_raw==G(SOURCE,historical)
before[historical]=historic_raw;outputs[historical]=historic_raw
records=[]
for path,b in before.items():
 archive=REL+'/role-metadata-before/'+path.replace('/','__')+'.txt';(a.out/path.replace('/','__')).write_bytes(b);records.append(dict(path=path,before=dict(length=len(b),sha256=H(b),archive=archive),after=dict(length=len(outputs[path]),sha256=H(outputs[path])),inverse=dict(require_complete_after_sha256=H(outputs[path]),restore_whole_before=archive)))
receipt=dict(status='PASS_SOURCE_ONLY_FINITE_ROLE_METADATA',source_head=a.expected_head,source_checkpoint=SOURCE,whole_before_base=BASE,new_modules=sorted(NEW),parsed_registration_parents=sorted(PARENTS),old_owners=220,current_owners=len(owners),full_root_enumeration=False,literal_grammar_transports=grammar_receipts,actual_source_bytes_never_normalized=True,original_parser_sha256=H(parser),original_generator_sha256=H(gen),original_boundary_tools_sha256=H(tool),all_other_roles_preserved=True,role_reclassifications=[dict(scope=name,source=path,before=prior['role'],after=m['classes'][path]['role'])for name,m in full.items()for path,prior in json.loads(before[F+name+'.json'])['classes'].items()if prior['role']!=m['classes'][path]['role']],old_ordered_chunks_preserved=True,historical_generic_proof_preserved_unchanged=True,global_generic_purity_result_not_claimed=True,distinct_current_boundary_proof=B+'boundary-source-proof.json',negative_controls=proof['negative_controls'],whole_inverses=records,written=a.write,Cargo=False)
(a.out/'receipt.json').write_bytes(enc(receipt))
if a.write:
 # Final source and tool identity guard before the first persistent write.
 assert git('rev-parse','HEAD').decode().strip()==a.expected_head
 assert not git('status','--porcelain','--untracked-files=all').strip()
 for model in full.values():
  for path,pin in model['sources'].items():
   actual=held(path,9753094);assert(len(actual),H(actual))==(pin['length'],pin['sha256']),path
 for path,initial in list(SNAPSHOTS.items()):assert held(path,max(1,len(initial)))==initial,path
 assert held(parserpath)==parser and held(genpath)==gen and held(toolpath)==tool
 assert git('rev-parse','HEAD').decode().strip()==a.expected_head and not git('status','--porcelain','--untracked-files=all').strip()
 for row in records:
  path=row['before']['archive'];q=R/path;q.parent.mkdir(parents=True,exist_ok=True);assert not q.exists();q.write_bytes(before[row['path']])
 for path,b in outputs.items():(R/path).write_bytes(b)
 (R/REL/'role-metadata-result.json').write_bytes(enc(receipt))
print(json.dumps({k:receipt[k]for k in ['status','source_head','old_owners','current_owners','negative_controls','written']}))
