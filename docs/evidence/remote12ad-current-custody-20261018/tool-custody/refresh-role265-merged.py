"""Finite same-role chain union of two actual published source generations."""
from pathlib import Path
import subprocess,os,stat,json,hashlib,copy,argparse
p=argparse.ArgumentParser();p.add_argument('--expected-head',required=True);p.add_argument('--out',type=Path,required=True);p.add_argument('--write',action='store_true');a=p.parse_args();R=Path('/Volumes/Semaprax/worktrees/next-large-combined-20261017');DC='dc64a29f847233f675507db16cdae41eaa347c1b';REMOTE='12adce43484c6bd914f03fbaebe1e11579dbccd2';F='docs/evidence/a3-e47-final-custody-20261016/';B='docs/evidence/source-boundary-domains-20261016/';REL='docs/evidence/remote12ad-current-custody-20261018';H=lambda b:hashlib.sha256(b).hexdigest();enc=lambda v:(json.dumps(v,indent=2)+'\n').encode();git=lambda *v:subprocess.check_output(['git',*v],cwd=R);cache={};snap={}
def G(rev,path):
 key=(rev,path)
 if key not in cache:cache[key]=git('show',rev+':'+path)
 return cache[key]
def held(path,cap=9753094):
 fd=os.open(R/path,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC)
 try:
  st=os.fstat(fd);ident=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns);assert stat.S_ISREG(st.st_mode)and st.st_size<=cap
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(st.st_size);assert len(b)==st.st_size and not f.read(1)
  assert ident(st)==ident(os.fstat(fd))==ident(os.lstat(R/path));assert b==G(a.expected_head,path)
  if path in snap:assert b==snap[path]
  else:snap[path]=b
  return b
 finally:os.close(fd)
def clean():assert git('rev-parse','HEAD').decode().strip()==a.expected_head and not git('status','--porcelain','--untracked-files=all').strip()
LINT_PARENTS=['src/cleanup_plan/replay/sequence_prefix/main24/wrapper_context.rs', 'src/cleanup_plan/replay/sequence_prefix/main8a/wrapper_context.rs', 'src/cleanup_plan/replay/sequence_prefix/main_c11/wrapper_context.rs']
LINT_ATTRIBUTE='#[allow(\n    clippy::module_inception,\n    reason = "The nested control deliberately reproduces the predecessor lexical module path."\n)]'
def shape(ch):
 c=copy.deepcopy(ch)
 for e in c['edges']:
  e.pop('start');e.pop('text')
  if e['parent']in LINT_PARENTS and LINT_ATTRIBUTE in e['attributes']:e['attributes'].remove(LINT_ATTRIBUTE)
 return c
moves=[]
def current_chain(ch,rev):
 c=copy.deepcopy(ch)
 for e in c['edges']:
  old=G(rev,e['parent']);text=e['text'].encode();assert old[e['start']:e['start']+len(text)]==text
  now=held(e['parent'])
  if now[e['start']:e['start']+len(text)]==text:continue
  # The same whole old/new parent and same ordered literal occurrences establish
  # a typed offset successor; no role/attribute/path or edge text is invented.
  assert old.count(text)==now.count(text)>0,(e['parent'],e['child'],'literal cardinality changed')
  ordinal=old[:e['start']].count(text);positions=[];start=0
  while True:
   at=now.find(text,start)
   if at<0:break
   positions.append(at);start=at+len(text)
  start=positions[ordinal];moves.append(dict(parent=e['parent'],child=e['child'],before=e['start'],after=start,text_sha256=H(text),prior_generation=rev));e['start']=start;assert now[start:start+len(text)]==text
 return c
clean();assert not a.out.exists();a.out.mkdir(parents=True);before={};full={};changed=set();extra={}
for name in ['hir','projection']:
 path=F+name+'.json';before[path]=held(path,16777216);assert before[path]==G(DC,path);old=json.loads(before[path]);remote=json.loads(G(REMOTE,path));m=copy.deepcopy(old)
 assert old['classes'].keys()==remote['classes'].keys() and old['chunks']==remote['chunks'] and old['sources'].keys()==remote['sources'].keys()
 restore=copy.deepcopy(remote);restore['classes']=old['classes'];restore['sources']=old['sources'];assert restore==old
 for source,row in m['sources'].items():
  b=held(source)
  if (len(b),H(b))!=(row['length'],row['sha256']):changed.add(source);row.update(length=len(b),sha256=H(b))
 extras=[]
 for source,cls in m['classes'].items():
  prior=old['classes'][source];other=remote['classes'][source];x=copy.deepcopy(other);x['chains']=prior['chains'];assert x==prior
  used=set();chains=[]
  for ch in prior['chains']:
   choices=[(i,c)for i,c in enumerate(other['chains'])if i not in used and shape(c)==shape(ch)]
   if choices:
    i,c=choices[0];used.add(i)
    for left,right in zip(ch['edges'],c['edges']):
     if left['text']!=right['text']:
      assert left['parent']in LINT_PARENTS and right['parent']==left['parent']
      assert right['text']==left['text'].replace('pub(super) mod before_wrapper;',LINT_ATTRIBUTE+'\npub(super) mod before_wrapper;',1)
      assert right['attributes']==left['attributes']+[LINT_ATTRIBUTE]
      original=G(DC,left['parent']);incoming=G(REMOTE,left['parent']);assert incoming.count((LINT_ATTRIBUTE+'\n').encode())==1 and incoming.replace((LINT_ATTRIBUTE+'\n').encode(),b'',1)==original
     else:assert left['text']==right['text'] and left['attributes']==right['attributes']
    chains.append(current_chain(c,REMOTE))
   else:chains.append(current_chain(ch,DC))
  for i,ch in enumerate(other['chains']):
   if i in used:continue
   c=current_chain(ch,REMOTE)
   if c not in chains:chains.append(c);extras.append(source)
  cls['chains']=chains
  assert cls['role']==prior['role']==other['role']
  # Every old dc chain has its exact typed successor in original order.
  assert [shape(c)for c in chains[:len(prior['chains'])]]==[shape(c)for c in prior['chains']]
  assert all(shape(c)in [shape(x)for x in chains]for c in other['chains'])
  assert all(held(e['parent'])[e['start']:e['start']+len(e['text'].encode())]==e['text'].encode()for ch in chains for e in ch['edges'])
 extra[name]=dict(classes=sorted(set(extras)),chains=len(extras));full[name]=m
path=F+'new-owners.json';before[path]=held(path);assert before[path]==G(DC,path);owners=json.loads(before[path]);assert len(owners)==265 and len(json.loads(G(REMOTE,path)))==260
for row in owners:
 b=held(row['source']);row.update(sha256=H(b),lines=len(b.splitlines()))
toolpath=B+'boundary_tools.py';tool=held(toolpath);assert tool==G(DC,toolpath)==G(REMOTE,toolpath);ns={'__file__':str(R/toolpath)};exec(compile(tool,toolpath,'exec'),ns)
gp=F+'generate-authority.py';gen=held(gp);assert gen==G(DC,gp)==G(REMOTE,gp);prefix=gen.decode().split("for name,chunks in [('hir',hir_chunks),('projection',projection_chunks)]:",1)[0];fd=a.out/'full';fd.mkdir()
for name,m in full.items():(fd/(name+'.json')).write_text(json.dumps(m,separators=(',',':'))+'\n')
manifest,proof=ns['generated_scope'](R,fd,regenerated=(full,H(prefix.encode())));negative=ns['negative_checks'](R,fd,manifest);assert all(negative.values());proof.update(status='SOURCE_ONLY_EXACT_BOUNDARIES_AND_COMPLETE_SOURCE_ROLE_CUSTODY_PASS',negative_controls=negative,registration_update='PRESERVED_DC265_AND_INCOMING_SAME_ROLE_CHAINS_WITH_AUTHENTICATED_PHYSICAL_EDGE_SUCCESSORS',global_polluted_purity_result_not_claimed=True)
for path in [B+'current-domains.json',B+'boundary-source-proof.json',F+'authority-source-proof.json']:before[path]=held(path,16777216)
outputs={F+name+'.json':(fd/(name+'.json')).read_bytes()for name in full};outputs.update({F+'new-owners.json':enc(owners),B+'current-domains.json':enc(manifest),B+'boundary-source-proof.json':enc(proof),F+'authority-source-proof.json':before[F+'authority-source-proof.json']})
receipt=dict(status='PASS_SOURCE_ONLY_MERGED_SAME_ROLE_CUSTODY',head=a.expected_head,dc_base=DC,incoming_base=REMOTE,owners=265,incoming_owner_list=260,changed_sources=sorted(changed),incoming_additional_chains=extra,physical_edge_successors=moves,negative_controls=negative,whole_class_keys_chunks_roles_and_dc_owner_prefix_preserved=True,Cargo=False,written=a.write)
(a.out/'receipt.json').write_bytes(enc(receipt));clean()
for path,b in list(snap.items()):assert held(path,max(1,len(b)))==b
clean()
if a.write:
 for path,raw in before.items():
  dest=R/REL/'models-before'/path.replace('/','__');dest.parent.mkdir(parents=True,exist_ok=True);assert not dest.exists();dest.write_bytes(raw)
  incoming=R/REL/'incoming-models'/path.replace('/','__');incoming.parent.mkdir(parents=True,exist_ok=True);incoming.write_bytes(G(REMOTE,path))
 for path,b in outputs.items():(R/path).write_bytes(b)
 (R/REL/'role-result.json').write_bytes(enc(receipt))
print(json.dumps({k:receipt[k]for k in ['status','owners','incoming_owner_list','incoming_additional_chains','negative_controls','written']}))
