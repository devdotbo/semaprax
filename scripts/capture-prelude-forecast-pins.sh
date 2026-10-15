#!/bin/sh
# Runs only root-built owning pin producers. Never builds or edits source pins.
set -eu
if [ "$#" -lt 3 ] || [ "$#" -gt 4 ]; then
    echo "usage: $0 <absolute-lib-test-binary> <absolute-workspace-test-binary> <fresh-absolute-output-dir> [absolute-wasm-test-binary]" >&2
    exit 2
fi
lib_binary=$1
workspace_binary=$2
capture_root=$3
wasm_binary=${4:-}
for binary in "$lib_binary" "$workspace_binary" "$wasm_binary"; do
    test -n "$binary" || continue
    case "$binary" in /*) ;; *) echo "test binary must be absolute" >&2; exit 2 ;; esac
    test -x "$binary"
done
case "$capture_root" in /*) ;; *) echo "capture directory must be absolute" >&2; exit 2 ;; esac
mkdir "$capture_root"
mkdir "$capture_root/wires"
export SEMAPRAX_PRELUDE_PIN_CAPTURE_DIR="$capture_root/wires"
git rev-parse HEAD > "$capture_root/source-head.txt"
git diff --binary HEAD > "$capture_root/source-local.patch"
python3 - "$capture_root/binaries.json" "$lib_binary" "$workspace_binary" "$wasm_binary" <<'PY'
import hashlib,json,pathlib,sys
records=[]
for name in sys.argv[2:]:
    if not name:continue
    p=pathlib.Path(name)
    records.append(dict(path=str(p),sha256=hashlib.sha256(p.read_bytes()).hexdigest()))
pathlib.Path(sys.argv[1]).write_text(json.dumps(records,indent=2)+'\n')
PY
run_owner() {
    label=$1
    binary=$2
    selector=$3
    shift 3
    result=0
    "$binary" "$selector" --exact --nocapture --test-threads=1 "$@" > "$capture_root/$label.log" 2>&1 || result=$?
    printf '%s\t%s\t%s\n' "$label" "$result" "$selector" >> "$capture_root/status.tsv"
}
run_owner public-graph "$workspace_binary" semantic_graph::public_api_cli_bytes_getters_and_read_only_locking_are_exact
run_owner analysis "$workspace_binary" semantic_graph::public_workspace_analysis_api_cli_kats_and_locking_are_exact
run_owner projection-wire "$lib_binary" workspace_graph::type_proof_tests::projection_tests::emit_projection_wire_pin_candidate --ignored
if [ -n "$wasm_binary" ]; then
    run_owner calculator "$wasm_binary" scalar_browser_ci_contract::browser_known_answers_match_authenticated_baseline_and_rename_graphs
fi
python3 - "$capture_root" "$wasm_binary" <<'PY'
import hashlib,json,pathlib,sys
root=pathlib.Path(sys.argv[1]);wire_dir=root/'wires'
marker='PROJECTION_WIRE_PIN_CANDIDATE '
lines=(root/'projection-wire.log').read_text().splitlines()
wires=[line.split(marker,1)[1] for line in lines if marker in line]
if len(wires)==1:
    data=wires[0].encode();json.loads(data)
    with (wire_dir/'projection-wire.json').open('xb') as f:f.write(data)
expected=['public-graph','context','forward','capability-context','type-context','impact','custom-impact','review','capability-review','projection-wire']
if sys.argv[2]:expected+=['calculator-baseline','calculator-renamed']
records=[];missing=[]
for name in expected:
    path=wire_dir/(name+'.json')
    if not path.is_file():missing.append(name);continue
    data=path.read_bytes();value=json.loads(data)
    record=dict(name=name,file=str(path.relative_to(root)),bytes=len(data),document_sha256='sha256:'+hashlib.sha256(data).hexdigest())
    for field in ['graph_digest','workspace_revision','project_revision']:
        if field in value:record[field]=value[field]
    if 'budget' in value:record['budget']=value['budget']
    records.append(record)
report=dict(schema='semaprax.prelude-forecast-pin-candidates.v1',source_head=(root/'source-head.txt').read_text().strip(),source_patch_sha256=hashlib.sha256((root/'source-local.patch').read_bytes()).hexdigest(),candidates=records,missing=missing,note='Actual byte captures only; source pins were not edited. Inspect status.tsv and full logs. A missing renamed calculator capture can follow the earlier baseline pin assertion; review/update only captured baseline pins, then rerun into a fresh directory.')
(root/'candidates.json').write_text(json.dumps(report,indent=2)+'\n')
print(root/'candidates.json')
# An incomplete capture is explicitly non-success; ordinary pin assertion exit
# codes remain recorded separately, even when every requested wire was captured.
if missing:raise SystemExit(3)
PY
