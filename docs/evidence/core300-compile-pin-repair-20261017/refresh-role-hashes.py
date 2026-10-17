"""Exact unchanged-role/edge successor after the one-item reexport and FINAL6."""
from pathlib import Path
import argparse,json,hashlib,subprocess,os,stat,copy,difflib,re
BASE='a6c92a197a21ed3e8c67c842b20886f2240cb0e2';REL='docs/evidence/core300-compile-pin-repair-20261017';F='docs/evidence/a3-e47-final-custody-20261016/';B='docs/evidence/source-boundary-domains-20261016/';H=lambda b:hashlib.sha256(b).hexdigest();enc=lambda v:(json.dumps(v,indent=2)+'\n').encode()
p=argparse.ArgumentParser();p.add_argument('--repo',type=Path,required=True);p.add_argument('--expected-head',required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--write',action='store_true');a=p.parse_args();R=a.repo.resolve();assert R==Path('/Volumes/Semaprax/worktrees/large-next-implementation-integration/AI-Lang');git=lambda *x:subprocess.check_output(['git',*x],cwd=R);assert git('rev-parse','HEAD').decode().strip()==a.expected_head and not git('status','--porcelain').strip();assert not a.out.exists();a.out.mkdir(parents=True);snap={}
def held(path,cap=9753094):
 fd=os.open(R/path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC);ident=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  st=os.fstat(fd);assert stat.S_ISREG(st.st_mode)and st.st_size<=cap
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(st.st_size);assert len(b)==st.st_size and not f.read(1)
  assert ident(st)==ident(os.fstat(fd))==ident(os.lstat(R/path))
  if path in snap:assert snap[path]==b
  else:snap[path]=b
  return b
 finally:os.close(fd)

# Same declarations/classes/chunks; only exact byte offsets move after known
# import/attribute/pin edits. No parser or registration inference is invoked.
transforms={}
def remap(parent,at,text):
 old=git('show',BASE+':'+parent);now=held(parent);assert old[at:at+len(text)]==text
 if old==now:return at
 assert parent in {'src/cleanup_plan/replay/factored/canonical_shared/paid_route_tests.rs', 'src/source_reflows/semantic_receivers/status_inline.rs', 'src/source_reflows_merge/main_e177_final_inputs.rs', 'src/cleanup_plan/replay/expression_facts/receiving/directory/before_cursor.rs', 'src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs', 'src/source_reflows/incoming090_semantic_receivers.rs', 'src/source_reflows_merge/main_e177_inputs.rs', 'src/cleanup_plan/replay/factored/canonical_shared/paid_many/lexical_tests.rs', 'src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs'}
 if parent not in transforms:
  left=old.splitlines(True);right=now.splitlines(True);lo=[0];ro=[0]
  for x in left:lo.append(lo[-1]+len(x))
  for x in right:ro.append(ro[-1]+len(x))
  transforms[parent]=[(lo[i],lo[i+n],ro[j]-lo[i])for i,j,n in difflib.SequenceMatcher(a=left,b=right,autojunk=False).get_matching_blocks()if n]
 candidates=[at+d for x,y,d in transforms[parent]if x<=at and at+len(text)<=y]
 assert len(candidates)==1,(parent,at,text);result=candidates[0];assert now[result:result+len(text)]==text;return result

before={};full={};changed=set()
for name in ['hir','projection']:
 path=F+name+'.json';raw=held(path,16777216);assert raw==git('show',BASE+':'+path);before[path]=raw;m=json.loads(raw);old=copy.deepcopy(m)
 for source,row in m['sources'].items():
  current=held(source)
  if(len(current),H(current))!=(row['length'],row['sha256']):
   prior=git('show',BASE+':'+source);assert(len(prior),H(prior))==(row['length'],row['sha256']);assert source in {'src/cleanup_plan/replay/factored/canonical_shared/paid_route_tests.rs', 'src/source_reflows/semantic_receivers/status_inline.rs', 'src/source_reflows_merge/main_e177_final_inputs.rs', 'src/cleanup_plan/replay/expression_facts/receiving/directory/before_cursor.rs', 'src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs', 'src/source_reflows/incoming090_semantic_receivers.rs', 'src/source_reflows_merge/main_e177_inputs.rs', 'src/cleanup_plan/replay/factored/canonical_shared/paid_many/lexical_tests.rs', 'src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs'},source
   changed.add(source);row.update(length=len(current),sha256=H(current))
 for cls in m['classes'].values():
  for chain in cls['chains']:
   for edge in chain['edges']:
    raw=held(edge['parent']);text=edge['text'].encode();edge['start']=remap(edge['parent'],edge['start'],text);assert raw[edge['start']:edge['start']+len(text)]==text
 check=copy.deepcopy(m);check['sources']=old['sources']
 for key,cls in check['classes'].items():
  for chain,original_chain in zip(cls['chains'],old['classes'][key]['chains']):
   for edge,original_edge in zip(chain['edges'],original_chain['edges']):edge['start']=original_edge['start']
 assert check==old;full[name]=m
path=F+'new-owners.json';raw=held(path);assert raw==git('show',BASE+':'+path);before[path]=raw;owners=json.loads(raw);assert len(owners)==230
for row in owners:
 b=held(row['source'])
 if (H(b),len(b.splitlines()))!=(row['sha256'],row['lines']):
  assert row['source'] in {'src/cleanup_plan/replay/factored/canonical_shared/paid_route_tests.rs', 'src/source_reflows/semantic_receivers/status_inline.rs', 'src/source_reflows_merge/main_e177_final_inputs.rs', 'src/cleanup_plan/replay/expression_facts/receiving/directory/before_cursor.rs', 'src/cleanup_plan/replay/expression_facts/receiving/directory/status_prefix.rs', 'src/source_reflows/incoming090_semantic_receivers.rs', 'src/source_reflows_merge/main_e177_inputs.rs', 'src/cleanup_plan/replay/factored/canonical_shared/paid_many/lexical_tests.rs', 'src/cleanup_plan/replay/skeleton_work/enumerated_prefix/tests.rs'},('foreign changed owner',row['source'])
 row.update(sha256=H(b),lines=len(b.splitlines()))
toolpath=B+'boundary_tools.py';tool=held(toolpath);assert tool==git('show',BASE+':'+toolpath);ns={'__file__':str(R/toolpath)};exec(compile(tool,toolpath,'exec'),ns)
gp=F+'generate-authority.py';gen=held(gp);assert gen==git('show',BASE+':'+gp);prefix=gen.decode().split("for name,chunks in [('hir',hir_chunks),('projection',projection_chunks)]:",1)[0]
fd=a.out/'full';fd.mkdir()
for name,m in full.items():(fd/(name+'.json')).write_text(json.dumps(m,separators=(',',':'))+'\n')
manifest,proof=ns['generated_scope'](R,fd,regenerated=(full,H(prefix.encode())));proof['negative_controls']=ns['negative_checks'](R,fd,manifest);proof['status']='SOURCE_ONLY_EXACT_BOUNDARIES_AND_COMPLETE_SOURCE_ROLE_CUSTODY_PASS';proof['registration_update']='ONLY_NINE_CLOSED_SOURCE_HASHES_AND_EXACT_EDGE_OFFSETS_NO_ROLE_CHUNK_DECLARATION_OR_REGISTRATION_CHANGE';proof['global_polluted_purity_result_not_claimed']=True
for path in [B+'current-domains.json',B+'boundary-source-proof.json',F+'authority-source-proof.json']:before[path]=held(path,16777216);assert before[path]==git('show',BASE+':'+path)
outputs={F+name+'.json':(fd/(name+'.json')).read_bytes()for name in full};outputs[F+'new-owners.json']=enc(owners);outputs[B+'current-domains.json']=enc(manifest);outputs[B+'boundary-source-proof.json']=enc(proof);outputs[F+'authority-source-proof.json']=before[F+'authority-source-proof.json'];rows=[]
for path,raw in before.items():
 archive=REL+'/role-before/'+path.replace('/','__')+'.txt';rows.append(dict(source=path,before=dict(length=len(raw),sha256=H(raw),archive=archive),after=dict(length=len(outputs[path]),sha256=H(outputs[path])),inverse=dict(require_exact_after_sha256=H(outputs[path]),restore_whole_before=archive)));(a.out/path.replace('/','__')).write_bytes(raw)
receipt=dict(status='PASS_SOURCE_ONLY_HASH_SUCCESSOR',head=a.expected_head,base=BASE,changed_sources=sorted(changed),roles_chunks_declarations_attributes_unchanged=True,exact_registered_edge_offsets_refreshed=sorted(transforms),current_owners=230,physical174_membership_unchanged=True,incoming471_membership_unchanged=True,old_afa54_and454_unchanged=True,historical_generic_proof_unchanged=True,negative_controls=proof['negative_controls'],whole_inverses=rows,written=a.write,Cargo=False)
(a.out/'receipt.json').write_bytes(enc(receipt))
if a.write:
 assert git('rev-parse','HEAD').decode().strip()==a.expected_head and not git('status','--porcelain','--untracked-files=all').strip()
 for path,raw in list(snap.items()):assert held(path,max(1,len(raw)))==raw
 assert git('rev-parse','HEAD').decode().strip()==a.expected_head and not git('status','--porcelain','--untracked-files=all').strip()
 for path,row in outputs.items():
  archive=next(x['before']['archive']for x in rows if x['source']==path);q=R/archive;q.parent.mkdir(parents=True,exist_ok=True);assert not q.exists();q.write_bytes(before[path]);(R/path).write_bytes(row)
 (R/REL/'role-hash-result.json').write_bytes(enc(receipt))
print(json.dumps({k:receipt[k]for k in ['status','changed_sources','current_owners','negative_controls','written']}))
