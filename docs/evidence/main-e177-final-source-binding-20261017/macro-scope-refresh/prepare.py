"""One exact historical macro-scope acquisition step; no binder, parser, Cargo or repository writes."""
from pathlib import Path
import argparse,ast,copy,hashlib,json,os,stat,subprocess
P=argparse.ArgumentParser();P.add_argument('--repo',type=Path,required=True);P.add_argument('--expected-head',required=True);P.add_argument('--out',type=Path,required=True);a=P.parse_args();R=a.repo.resolve();O=a.out
G=lambda *x:subprocess.check_output(['git',*x],cwd=R);H=lambda b:hashlib.sha256(b).hexdigest();E=lambda v:(json.dumps(v,indent=2)+'\n').encode()
assert G('rev-parse','HEAD').decode().strip()==a.expected_head and not G('status','--porcelain').strip();assert not O.exists();O.mkdir(parents=True)
S='docs/evidence/main-e177-final-source-binding-20261017/';M=S+'current-inputs.json';F='src/source_reflows_merge/main_e177_final_inputs.rs';Q=S+'guard_payload.py';W=S+'successor174/bind-next.py'
BOOT=['docs/evidence/accepted-strict-guard-generation-20261017/check-source.py','docs/evidence/accepted-final6-source-binding-20261017/check-source.py','docs/evidence/main043-independent-custody-20261016/verify-or-refresh.py']
seen={};out={}
def held(p):
 fd=os.open(R/p,os.O_RDONLY|os.O_NOFOLLOW|os.O_NONBLOCK|os.O_CLOEXEC);ident=lambda s:(s.st_dev,s.st_ino,s.st_mode,s.st_size,s.st_mtime_ns,s.st_ctime_ns)
 try:
  st=os.fstat(fd);assert stat.S_ISREG(st.st_mode)and st.st_size<1000000
  with os.fdopen(fd,'rb',closefd=False)as f:b=f.read(st.st_size);assert len(b)==st.st_size and not f.read(1)
  assert ident(st)==ident(os.fstat(fd))==ident(os.lstat(R/p));assert b==G('show',a.expected_head+':'+p)
  if p in seen:assert seen[p]==b
  seen[p]=b;return b
 finally:os.close(fd)
def pin(b):return dict(length=len(b),sha256=H(b))
def replace(b,old,new):assert b.count(old)==1;(None);return b.replace(old,new,1)
def assign(b,name,value):
 n=next(n for n in ast.parse(b).body if isinstance(n,ast.Assign)and any(isinstance(t,ast.Name)and t.id==name for t in n.targets));lines=b.decode().splitlines(True);old=''.join(lines[n.lineno-1:n.end_lineno]).encode();return replace(b,old,(name+'='+repr(value)+'\n').encode())
old=held(M);assert len(old)==82192 and H(old)=='a1e425d6dec0ae2619b7c2f6964966a15e287aeecbdc27507166f13245a75fa6';m=json.loads(old);row=next(r for r in m['rows']if r['source']=='src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/before_borrowed_single.rs');prior=copy.deepcopy(row['current']);assert row['compiled']is True and prior['length']==15553
prior_source=held(prior['archive']);assert H(prior_source)==prior['sha256']
current=held('src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/before_borrowed_single.rs');assert len(current)==18228 and H(current)=='390fc54eb4796cd77e09a7cf31c2134b1115c9006a0b222b4e555c2bcb4a6250'
fix=json.loads(held('docs/evidence/borrowed-single-macro-scope-fix-20261017/inverse.json'));insert=fix['insertion'].encode();assert current.count(insert)==1 and current.replace(insert,b'',1)==prior_source;assert H(prior_source)==fix['before_sha256']and H(current)==fix['after_sha256']
archive=S+'current/'+H(current)+'.txt';out[archive]=current;row['current']={**pin(current),'archive':archive};row['views'].append(dict(name='whole-pre-macro-scope-current',**prior))
anchor=S+'views/'+H(old)+'.txt';out[anchor]=old;m['complete_old_table_anchors'].append(dict(path=anchor,**pin(old),archive=anchor));out[M]=E(m);assert len(out[M])<=262144
check=copy.deepcopy(m);r=next(r for r in check['rows']if r['source']=='src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/before_borrowed_single.rs');r['current']=prior;r['views'].pop();check['complete_old_table_anchors'].pop();assert check==json.loads(old)
b=held(F);b=replace(b,b'assert_eq!(data.len(), 82192);',('assert_eq!(data.len(), '+str(len(out[M]))+');').encode());b=replace(b,('assert_eq!(digest(data.as_bytes()), "'+H(old)+'");').encode(),('assert_eq!(digest(data.as_bytes()), "'+H(out[M])+'");').encode());line=('        "'+anchor+'" => Some(include_str!("../../'+anchor+'")),\n').encode();b=replace(b,b'fn table_compiled(source: &str) -> Option<&\'static str> {\n    match source {\n',b'fn table_compiled(source: &str) -> Option<&\'static str> {\n    match source {\n'+line);out[F]=b
qb=held(Q);cfg=ast.literal_eval(next(n.value for n in ast.parse(qb).body if isinstance(n,ast.Assign)and any(isinstance(t,ast.Name)and t.id=='CONFIG'for t in n.targets)));assert 'CHANGELOG.md'not in cfg['closed_prior_sources'];closed=copy.deepcopy(cfg['closed_prior_sources']);cfg['packet'].update(pin(out[M]));assert F in cfg['modules'];cfg['modules'][F].update(pin(out[F]));cfg['modules']['src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/before_borrowed_single.rs'].update(pin(current));assert cfg['closed_prior_sources']==closed;out[Q]=assign(qb,'CONFIG',cfg)
for p in BOOT:
 b=held(p);b=replace(b,str(len(qb)).encode(),str(len(out[Q])).encode());b=replace(b,H(qb).encode(),H(out[Q]).encode());out[p]=b
# Original executed successor174 and frozen174-preparation remain immutable prior-head evidence.
held(W);held(S+'frozen174-preparation.json')
records=[]
for p,b in sorted(out.items()):
 q=O/'candidate'/p;q.parent.mkdir(parents=True,exist_ok=True);q.write_bytes(b)
 priorbytes=held(p)if (R/p).exists()else None
 if priorbytes is not None:
  z=O/'before'/p;z.parent.mkdir(parents=True,exist_ok=True);z.write_bytes(priorbytes)
 records.append(dict(path=p,before=None if priorbytes is None else pin(priorbytes),after=pin(b),inverse='restore complete before; remove only if previously absent'))
assert G('rev-parse','HEAD').decode().strip()==a.expected_head and not G('status','--porcelain').strip()
for p,b in list(seen.items()):assert held(p)==b
(O/'receipt.json').write_bytes(E(dict(status='PREPARED_SOURCE_ONLY_NOT_APPLIED',source_head=a.expected_head,current_rows=98,unchanged_rows=97,compiled_true_preserved=True,Python_channels_unchanged=closed,whole_prior_table_anchor=anchor,changed_STRICT=[],STRICT_added=[],historical_174_wrapper_preserved=True,prior_174_actual_head='f71d7a308aa2dbbf5b21c456b399a5cffec48aac',prior_FINAL6_actual_head='a4fba2df26877b5f7e1caa2f2d478c9d9df50545',physical_binders_not_executed=True,role_refresh_required=[F,'src/cleanup_plan/replay/factored/canonical_shared/lexical_successors/frame_loop/before_borrowed_single.rs'],files=records)))
print(json.dumps(dict(files=len(records),packet=pin(out[M]),guard=pin(out[Q]),helper=pin(out[F]))))
