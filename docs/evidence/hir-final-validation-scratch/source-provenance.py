"""Static complete source/body proof only; never compiler or test execution."""
from pathlib import Path
import hashlib,json,re,subprocess
ROOT=Path(__file__).resolve().parents[3]
OUT=Path(__file__).resolve().parent
BASE="1a680e93e"
TOKEN=re.compile(r'r(?P<hashes>\#{0,})"[\s\S]*?"(?P=hashes)|"(?:\\[\s\S]|[^"\\])*"|//[^\n]*|/\*[\s\S]*?\*/|\w+|[^\s]')
def tokens(s):return [m.group() for m in TOKEN.finditer(s) if not m.group().startswith(("//","/*"))]
def body(s,name):
 t=tokens(s);start=next(i for i in range(len(t)-1) if t[i:i+2]==["fn",name]);start=t.index("{",start);depth=0
 for end in range(start,len(t)):
  depth+=(t[end]=="{")-(t[end]=="}")
  if depth==0:return t[start:end+1]
 raise ValueError(name)
def old(path):return subprocess.check_output(["git","show",BASE+":"+path],cwd=ROOT,text=True)
def current(path):return (ROOT/path).read_text()
def sha(s):return hashlib.sha256(s.encode()).hexdigest()
proof=[]
for original,method,receiver,receiving in [
 ("src/hir/validation/callable_types.rs","validate_type","src/hir/validation/callable_types/original_receiver.rs","validate_type_original_receiver"),
 ("src/hir/validation/nominal_facts.rs","prepare","src/hir/validation/nominal_facts.rs","prepare_owned_keys"),
 ("src/hir/declaration_index/flags/paid.rs","type_flags_paid","src/hir/declaration_index/flags/paid.rs","type_flags_paid_original_receiver"),
 ("src/hir/declaration_index/flags/paid.rs","type_flags_paid","src/hir/declaration_index/flags/paid.rs","type_flags_paid_construct"),
 ("src/hir/declaration_index/flags/paid.rs","flags_paid","src/hir/declaration_index/flags/paid.rs","flags_paid"),
 ("src/hir/nodes.rs","value_type","src/hir/nodes.rs","value_type"),
 ("src/hir/validation/callable_types.rs","validate_type_eager_oracle","src/hir/validation/callable_types.rs","validate_type_eager_oracle"),
 ("src/hir/validation/callable_types.rs","validate_declared_ownership_full_facts_oracle","src/hir/validation/callable_types.rs","validate_declared_ownership_full_facts_oracle"),
]:
 a=body(old(original),method);b=body(current(receiver),receiving);assert a==b,(original,method,receiver)
 proof.append(dict(original=original,method=method,receiver=receiver,receiving=receiving,tokens=len(a),sha256=sha(json.dumps(a))))
original_tests=[]
for path in ["src/hir/declaration_index/flags/tests.rs","src/hir/validation/nominal_facts/tests.rs","src/cleanup_plan/replay/paid_type_facts/tests.rs"]:
 for method in re.findall(r"#[\[]test[\]]\s*fn (\w+)\(",old(path)):
  before=body(old(path),method);after=body(current(path),method)
  if method=="complete_classifier_source_join_keeps_the_existing_payer_and_full_receiver_inventory":
   for child in ["../../../hir/nodes/import_result_type.rs","../../../hir/nodes/import_result_type/tests.rs"]:
    addition=tokens('include_str!("'+child+'"),');offset=next(i for i in range(len(after)) if after[i:i+len(addition)]==addition);after=after[:offset]+after[offset+len(addition):]
  assert before==after,(path,method,"original test weakened")
  original_tests.append(dict(path=path,method=method,tokens=len(before),sha256=sha(json.dumps(before))))
unchanged=[]
for path in ["src/hir/validation.rs","src/hir/declaration_index.rs","src/hir/function_value.rs"]:
 assert old(path)==current(path),path
 unchanged.append(dict(path=path,sha256=sha(current(path))))
baselines=[]
for path in sorted(set(row["original"] for row in proof)|set(row["path"] for row in unchanged)|set(row["path"] for row in original_tests)):
 content=old(path);baselines.append(dict(path=path,sha256=sha(content),content=content))
paths=subprocess.check_output(["git","diff","--name-only",BASE],cwd=ROOT,text=True).splitlines()
paths += subprocess.check_output(["git","ls-files","--others","--exclude-standard"],cwd=ROOT,text=True).splitlines()
sources=[]
for path in sorted(set(paths)):
 if path.endswith(".rs"):
  content=current(path);sources.append(dict(path=path,lines=len(content.splitlines()),sha256=sha(content),content=content))
budgets={}
for line in current("tests/module-size-budget.tsv").splitlines():
 if line and not line.startswith("#"):
  path,count=line.split("\t");budgets[path]=int(count)
files=set(ROOT.glob("src/**/*.rs"))|set(ROOT.glob("tests/**/*.rs"))|set(ROOT.glob("crates/*/src/**/*.rs"))|set(ROOT.glob("crates/*/tests/**/*.rs"))
violations=[];census=[]
for path in sorted(files):
 rel=path.relative_to(ROOT).as_posix();count=len(path.read_text().splitlines());cap=budgets.get(rel,1500)
 if count>cap:violations.append(dict(path=rel,lines=count,cap=cap))
 if rel in {row["path"] for row in sources} or count>1500:census.append(dict(path=rel,lines=count,cap=cap))
introduced=[row for row in violations if len(old(row["path"]).splitlines()) < row["lines"]]
assert not introduced,introduced
report=dict(base=subprocess.check_output(["git","rev-parse",BASE],cwd=ROOT,text=True).strip(),qualification="Static source/body and size census only; no Cargo/build/tests executed.",complete_baseline_sources=baselines,complete_current_sources=sources,body_receivers=proof,original_complete_tests=original_tests,unchanged_full_sources=unchanged,rust_file_count=len(files),size_rows=census,size_violations=violations,introduced_size_violations=introduced)
(OUT/"source-provenance.json").write_text(json.dumps(report,indent=2,sort_keys=True)+"\n")
print(f"Static proof: {len(proof)} complete bodies, {len(unchanged)} unchanged full sources, {len(sources)} complete current sources, {len(files)} Rust files, {len(violations)} inherited cap violations, zero introduced violations.")
