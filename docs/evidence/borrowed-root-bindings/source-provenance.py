"""Source-only complete frozen receiver/body and literal proof; no compiler/test invocation."""
from pathlib import Path
import hashlib,json,re,subprocess
ROOT=Path(__file__).resolve().parents[3];OUT=ROOT/'docs/evidence/borrowed-root-bindings';BASE='eb02a89b6174bced05e02f6ede2ebcda55cd5893'
TOKEN=re.compile(r'r(?P<hashes>\#{0,})"[\s\S]*?"(?P=hashes)|"(?:\\[\s\S]|[^"\\])*"|//[^\n]*|/\*[\s\S]*?\*/|\w+|[^\s]')
def tokens(s):return [m.group() for m in TOKEN.finditer(s) if not m.group().startswith(('//','/*'))]
def bodies(s):
 t=tokens(s);out=[]
 for i in range(len(t)-1):
  if t[i]!='fn' or not re.match(r'^\w+$',t[i+1]):continue
  start=i+2
  while start<len(t) and t[start] not in ('{',';'):start+=1
  if start==len(t) or t[start]==';':continue
  depth=0
  for end in range(start,len(t)):
   depth+=(t[end]=='{')-(t[end]=='}')
   if depth==0:out.append((t[i+1],t[start:end+1]));break
 return out
sha=lambda s:hashlib.sha256(s.encode()).hexdigest()
def frozen(path):return subprocess.check_output(['git','show',BASE+':'+path],cwd=ROOT,text=True)
proof=[]
for row in json.loads((OUT/'frozen-source-map.json').read_text()):
 path=row['source'];before=frozen(path);archive=OUT/(path.replace('/','__')+'.txt')
 assert archive.read_bytes()==before.encode(),path
 after=(ROOT/row['receiver']).read_text().replace('crate::hir::borrowed_root_oracle::','crate::hir::')
 a=bodies(before);b=bodies(after)
 assert a==b,(path,[(x[0],y[0]) for x,y in zip(a,b) if x!=y],len(a),len(b))
 proof.append(dict(source=path,receiver=row['receiver'],complete_function_bodies=len(a),frozen_sha256=sha(before),receiver_sha256=sha((ROOT/row['receiver']).read_text())))
# Complete unchanged services/types at intentional shared API boundaries; paid publication untouched.
unchanged=[]
for path in ['src/hir/resolve_expr/path_arena.rs','src/hir/closure.rs','src/hir/closure/resolve.rs','src/hir/closure/once.rs','src/hir/closure/mutable.rs','src/hir/closure/paid.rs','src/hir/closure/paid/copy.rs','src/hir/closure/paid/tests.rs','src/hir/closure/paid/original_tests.rs','src/hir/resolve_expr_reference.rs','src/hir/capacity_probe.rs']:
 before=frozen(path);after=(ROOT/path).read_text()
 if path=='src/hir/closure/resolve.rs':
  old='&DeclarationId::new(source_function.stable_id.clone())';held='&self.declarations.held_identity(&source_function.stable_id)'
  assert before.count(old)==1 and before.count(held)==0,path
  assert after.count(old)==0 and after.count(held)==1,path
  assert after==before.replace(old,held,1),path
  assert (OUT/(path.replace('/','__')+'.txt')).read_text()==before,path
 else:assert before==after,path
 # Original digest remains immutable, including the exact projected producer.
 unchanged.append(dict(path=path,sha256=sha(before)))
# The row schema itself remains the exact shared Binding payload ABI.
def declaration(s,name):
 t=tokens(s);i=next(i for i in range(len(t)-1) if t[i:i+2]==['struct',name]);start=t.index('{',i);depth=0
 for end in range(start,len(t)):
  depth+=(t[end]=='{')-(t[end]=='}')
  if depth==0:return t[start:end+1]
for path,name in [('src/hir.rs','Binding'),('src/hir/resolve_for/record.rs','Row')]:
 assert declaration(frozen(path),name)==declaration((ROOT/path).read_text(),name),(path,name)
# Every old fixture literal is byte-identical, and every unchanged assertion body is retained.
controls=[]
for path in ['src/hir/resolve_expr/capacity/tests.rs','src/hir/iterative_validator_tests.rs','src/hir/resolve_expr/path_arena_regression_tests.rs','src/hir/resolve_expr_frame.rs']:
 old=frozen(path);new=(ROOT/path).read_text()
 def literals(text):return [m.group() for m in TOKEN.finditer(text) if m.group().startswith(('"','r"','r#'))]
 old_literals=literals(old);new_literals=literals(new)
 # Old literals form an ordered subsequence; additive oracle messages are allowed.
 it=iter(new_literals)
 assert all(any(current==before for current in it) for before in old_literals),path
 controls.append(dict(path=path,old_literal_count=len(old_literals),all_old_literals_retained_in_order=True))
budgets={line.split('\t')[0]:int(line.split('\t')[1]) for line in (ROOT/'tests/module-size-budget.tsv').read_text().splitlines() if line and not line.startswith('#')}
changed=subprocess.check_output(['git','diff','--name-only',BASE],cwd=ROOT,text=True).splitlines()+subprocess.check_output(['git','ls-files','--others','--exclude-standard'],cwd=ROOT,text=True).splitlines()
sources=[]
for path in sorted(set(changed)):
 if path.endswith('.rs'):
  text=(ROOT/path).read_text();count=len(text.splitlines());assert count<=budgets.get(path,1500),(path,count,budgets.get(path,1500));sources.append(dict(path=path,lines=count,sha256=sha(text)))
report=dict(base=BASE,qualification='Static full function-body/literal/source and module-size proof only. No Cargo/build/tests executed.',frozen_receivers=proof,unchanged_full_sources=unchanged,old_fixture_literals=controls,current_sources=sources,shared_type_boundary='PathArena API and Row schema are byte-proven unchanged; carrier-sensitive iterative, scheduling, mutation and closure-body receivers are independent copies.')
(OUT/'source-provenance.json').write_text(json.dumps(report,indent=2)+'\n')
print(f'Static proof: {sum(x["complete_function_bodies"] for x in proof)} complete frozen bodies across {len(proof)} sources; {len(unchanged)} unchanged full services; {len(sources)} source size checks.')
