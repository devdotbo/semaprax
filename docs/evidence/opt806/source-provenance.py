"""Record static original-body/literal provenance; never execute compiler/tests."""
from pathlib import Path
import hashlib
import json
import re
import subprocess

ROOT = Path(__file__).resolve().parents[3]
BASE = "5728c9e48fe2afe5cd77e9edf000cbe171035bdb"
TOKEN = re.compile(r'r(?P<hashes>\#{0,})"[\s\S]*?"(?P=hashes)|"(?:\\[\s\S]|[^"\\])*"|\'(?:\\[\s\S]|[^\'\\])\'|//[^\n]*|/\*[\s\S]*?\*/|\w+|[^\s]')

ARCHIVE = json.loads((ROOT / "docs/evidence/opt806/baseline-sources.json").read_text())
assert ARCHIVE["base"] == BASE
ORIGINALS = {row["path"]: row for row in ARCHIVE["sources"]}
for row in ORIGINALS.values():
    assert hashlib.sha256(row["content"].encode()).hexdigest() == row["sha256"], row["path"]

def original(path):
    archived = ORIGINALS[path]["content"]
    exact_git = subprocess.check_output(["git", "show", f"{BASE}:{path}"], cwd=ROOT, text=True)
    assert archived == exact_git, (path, "complete original source drift")
    return archived

def tokens(text):
    return [m.group() for m in TOKEN.finditer(text) if not m.group().startswith(("//", "/*"))]

def body(text, method):
    stream = tokens(text)
    start = next(i for i in range(len(stream)-1) if stream[i:i+2] == ["fn", method])
    start = stream.index("{", start)
    depth = 0
    for end in range(start, len(stream)):
        depth += (stream[end] == "{") - (stream[end] == "}")
        if depth == 0:
            return stream[start:end+1]
    raise ValueError(method)

def without(stream, adaptation):
    adapted = tokens(adaptation)
    offset = next(i for i in range(len(stream)-len(adapted)+1) if stream[i:i+len(adapted)] == adapted)
    return stream[:offset] + stream[offset+len(adapted):]

def digest(stream):
    return hashlib.sha256(json.dumps(stream, ensure_ascii=False, separators=(",", ":")).encode()).hexdigest()

rows=[]
for leaf, methods in [
    ("enter", ["frame_enter"]),
    ("blocks", ["frame_resume_block_statement"]),
    ("operators", ["frame_resume_unary", "frame_resume_binary_right"]),
    ("records", ["frame_prepare_record_field"]),
    ("match_arms", ["frame_prepare_variant_match_arm"]),
]:
    baseline = f"src/source_verify/iterative/{leaf}.rs"
    saved = f"src/source_verify/recovery_legacy/{leaf}.rs"
    for method in methods:
        old = body(original(baseline), method)
        receiving = body((ROOT/saved).read_text(), "recovery_old_"+method)
        adaptations=[]
        if leaf == "enter":
            adaptation="recovery_checkpoint: self.recovery.checkpoint(self.diagnostics),"
            receiving=without(receiving, adaptation)
            adaptations.append(adaptation)
        assert receiving == old, (saved, method)
        rows.append(dict(original=baseline, saved=saved, method=method, body_tokens=len(old), sha256=digest(old), adaptations=adaptations))

saved="src/source_verify/recovery_legacy.rs"
text=(ROOT/saved).read_text()
for owner, method in [
    ("src/source_verify.rs", "new"),
    ("src/source_verify/iterative.rs", "check_expr_iterative"),
    ("src/source_verify/iterative.rs", "check_expr_with_helpers"),
    ("src/source_verify/diagnostics.rs", "require_bool"),
]:
    old=body(original(owner), method)
    receiving=body(text, "new_legacy" if method=="new" else method+"_legacy")
    receiving=[{"new_legacy":"new", "check_expr_with_helpers_legacy":"check_expr_with_helpers", "check_expr_iterative_legacy":"check_expr_iterative"}.get(t,t) for t in receiving]
    adaptations=[]
    if method == "new":
        adaptation="recovery: super::recovery::Recovery::default(), recovery_legacy: true,"
        receiving=without(receiving, adaptation)
        adaptations.append(adaptation)
    assert receiving == old, (saved,method)
    rows.append(dict(original=owner, saved=saved, method=method, body_tokens=len(old), sha256=digest(old), adaptations=adaptations))

unchanged=[]
for path in ["src/source_verify/oracle.rs", "src/source_verify/declaration/functions/old.rs"] + sorted(path for path in ORIGINALS if path.startswith("src/source_verify/oracle/")):
    if path.endswith("functions/old.rs"):
        for method in ["check_function_declarations", "check_function_bodies", "check_generic_function_cycles"]:
            old=body(original(path),method);current=body((ROOT/path).read_text(),method)
            assert old==current,(path,method)
            unchanged.append(dict(path=path, method=method, sha256=digest(old)))
    else:
        assert original(path)==(ROOT/path).read_text(),path
        unchanged.append(dict(path=path,sha256=hashlib.sha256((ROOT/path).read_bytes()).hexdigest()))

corpus="docs/evidence/opt806/proposal-corpus-original.rs.txt"
adapted="src/source_verify/recovery_tests/proposal_corpus.rs"
prior=(ROOT/corpus).read_text(); current=(ROOT/adapted).read_text()
old_cases=prior[prior.index("let cases:"):prior.index("    for (mistake,")]
new_cases=current[current.index("let cases:"):current.index("    assert_eq!(cases.len()")]
def literals(text):
    return [m.group() for m in TOKEN.finditer(text) if m.group().startswith(('"', 'r"', 'r#'))]
assert literals(old_cases)==literals(new_cases),"60-case authored literal drift"
report=dict(schema="semaprax.scoped-diagnostic-recovery-source-provenance.v1",base=BASE,
    proposal="d36d47daef42e5d7e06e16a7ebade20be66ba95e",body_receivers=rows,unchanged_receivers=unchanged,
    full_source_archive=dict(path="docs/evidence/opt806/baseline-sources.json",source_count=len(ORIGINALS),sha256=hashlib.sha256((ROOT/"docs/evidence/opt806/baseline-sources.json").read_bytes()).hexdigest()),
    corpus=dict(original=corpus,adapted=adapted,literal_count=len(literals(old_cases)),literal_sha256=digest(literals(old_cases)),original_sha256=hashlib.sha256((ROOT/corpus).read_bytes()).hexdigest()),
    qualification="Static source/literal provenance only. No Cargo, build or test execution.")
(ROOT/"docs/evidence/opt806/source-provenance.json").write_text(json.dumps(report,indent=2,sort_keys=True)+"\n")
print(f"Recorded {len(rows)} complete receiver bodies, {len(unchanged)} unchanged body/file rows and all 60-case literals.")
