#!/usr/bin/env python3
"""Plan/apply reviewed pins from complete actual ROOT producer captures only.

No compiler/build invocation, synthesized wire, browser update or gate bypass.
Default is read-only. --apply requires a fresh evidence directory.
"""
import argparse
import hashlib
import json
from pathlib import Path
import shutil

ANALYSIS = [
    ("context", "26296de75d133f1563b1b4fc23049b6eb29f4b407a2369d4849a12e9a379f225"),
    ("forward", "190a00dfa539b7cef26a50dcbda97d522bb22699b02c6b904682d524806266c8"),
    ("capability-context", "7ba1554e318ce8d56739e03a2987c90d5d7fdf1ff301b528ad67c1f8cd432a42"),
    ("type-context", "bccc97fe0ad0026b44544e1e7c27c72f4d266c6773de01e157dcc9492317dd83"),
    ("impact", "6e0ea8e0fc10e31b325fdc33e41ab9030b75f393c2f0e4fec916be6a19abab62"),
    ("custom-impact", "421e0b0fba5f10c294d13bbfde592c9837468e074cacf6b113ab62051299b3bc"),
    ("review", "864367757119c169b5f049542de643b2825bcc64794ae91784920e8d73d18faf"),
    ("capability-review", "812fa5e3a129b96f36eed9e686ec88e3b5118813f31106c42adae637c6c14b21"),
]
OWNERS = {
    "public-graph": "semantic_graph::public_api_cli_bytes_getters_and_read_only_locking_are_exact",
    "analysis": "semantic_graph::public_workspace_analysis_api_cli_kats_and_locking_are_exact",
    "projection-wire": "workspace_graph::type_proof_tests::projection_tests::emit_projection_wire_pin_candidate",
}

def sha(data):
    return "sha256:" + hashlib.sha256(data).hexdigest()

def require(condition, reason):
    if not condition:
        raise SystemExit(reason)

def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("capture", type=Path)
    parser.add_argument("--repo", required=True, type=Path)
    parser.add_argument("--expected-capture-head", required=True)
    parser.add_argument("--apply", action="store_true")
    parser.add_argument("--evidence-dir", type=Path)
    args = parser.parse_args()
    capture = args.capture.resolve()
    repo = args.repo.resolve()
    manifest = json.loads((capture / "candidates.json").read_text())
    require(manifest["schema"] == "semaprax.prelude-forecast-pin-candidates.v1", "wrong capture schema")
    require(not manifest["missing"], "incomplete capture; do not update pins")
    head = (capture / "source-head.txt").read_text().strip()
    require(len(args.expected_capture_head) >= 7 and head.startswith(args.expected_capture_head), "capture head differs from ROOT-approved build")
    require(head == manifest["source_head"], "capture head manifest mismatch")
    source_patch = (capture / "source-local.patch").read_bytes()
    require(not source_patch, "capture includes source drift; ROOT must audit and recapture frozen source")
    require(hashlib.sha256(source_patch).hexdigest() == manifest["source_patch_sha256"], "capture patch manifest mismatch")
    binaries = json.loads((capture / "binaries.json").read_text())
    require(len(binaries) >= 2, "missing built-binary provenance")
    statuses = {}
    for line in (capture / "status.tsv").read_text().splitlines():
        label, code, selector = line.split("\t")
        require(label not in statuses, "duplicate producer status")
        statuses[label] = (int(code), selector)
    for label, selector in OWNERS.items():
        require(label in statuses and statuses[label][1] == selector, "missing exact owning producer: " + label)
        require(statuses[label][0] in (0, 101), "non-test producer failure: " + label)
        log = (capture / (label + ".log")).read_text()
        require(selector in log and "running 1 test" in log, "not one exact owning test: " + label)
    require(statuses["projection-wire"][0] == 0, "ignored wire producer failed")
    names = ["public-graph"] + [name for name, _ in ANALYSIS] + ["projection-wire"]
    index = {record["name"]: record for record in manifest["candidates"]}
    require(len(index) == len(manifest["candidates"]), "duplicate wire candidate")
    wires = {}
    values = {}
    for name in names:
        require(name in index, "missing actual wire: " + name)
        data = (capture / "wires" / (name + ".json")).read_bytes()
        record = index[name]
        require(record["file"] == "wires/" + name + ".json", "unexpected candidate path")
        require(sha(data) == record["document_sha256"] and len(data) == record["bytes"], "actual bytes differ from manifest: " + name)
        require(not data.endswith(b"\n"), "wire has noncanonical terminal LF: " + name)
        values[name] = json.loads(data)
        wires[name] = data
    graph = values["public-graph"]
    require(graph["schema"] == "semaprax.workspace-semantic-graph.v1", "public graph schema drift")
    amount = graph["budget"]["used_builder_bytes"]
    require(type(amount) is int and 0 < amount <= 67108864, "actual builder amount outside unchanged cap")
    for name in ["public-graph", "projection-wire"]:
        value = values[name]
        digest = value["graph_digest"]
        field = (',"graph_digest":"' + digest + '"').encode()
        require(wires[name].count(field) == 1, "graph digest field framing drift")
        require(sha(wires[name].replace(field, b"", 1)) == digest, "graph digest does not bind full payload")
    for name, _ in ANALYSIS:
        require(values[name]["workspace_graph_digest"] == graph["graph_digest"], "analysis graph binding differs: " + name)
    edits = []
    def edit(path, before, after, evidence):
        edits.append(dict(path=path, before=before, after=after, actual_wire=evidence))
    owner = "tests/workspace/semantic_graph.rs"
    edit(owner, "assert_eq!(graph.budget().used_builder_bytes(), 334_016);",
         "assert_eq!(graph.budget().used_builder_bytes(), " + format(amount, "_") + ");", "public-graph")
    edit(owner, "sha256:c5cbb726c4e888d9b6e945964e52d233af45d25c80dc292890343d5ad2079a2e",
         sha(wires["public-graph"]), "public-graph")
    for name, old in ANALYSIS:
        edit(owner, "sha256:" + old, sha(wires[name]), name)
    edit("src/workspace_graph/type_proof_tests.rs",
         "sha256:ba3d1b86d584444b3ed9d18bb0dfd2ed7b69a2a68841a40de3ce5cdfc5dc39f9",
         sha(wires["projection-wire"]), "projection-wire")
    before_sources = {}
    after_sources = {}
    for path in sorted({record["path"] for record in edits}):
        before_sources[path] = (repo / path).read_text()
        after_sources[path] = before_sources[path]
    for record in edits:
        path = record["path"]
        require(after_sources[path].count(record["before"]) == 1, "old pin not unique/current: " + path)
        after_sources[path] = after_sources[path].replace(record["before"], record["after"], 1)
    report = dict(schema="semaprax.prelude-forecast-reviewed-pin-plan.v1", capture_head=head,
                  status="actual-candidate-plan-requires-wire-review", edits=edits,
                  source_sha256={path: sha(text.encode()) for path, text in before_sources.items()})
    print(json.dumps(report, indent=2))
    if not args.apply:
        return
    require(args.evidence_dir is not None, "--apply requires --evidence-dir")
    evidence = args.evidence_dir.resolve()
    require(not evidence.exists(), "evidence directory must be fresh")
    # All inputs, pins and replacements validate before creating any candidate.
    evidence.mkdir(parents=True)
    shutil.copytree(capture, evidence / "actual-capture")
    for path, text in before_sources.items():
        (evidence / (path.replace("/", "__") + ".before.txt")).write_text(text)
    (evidence / "reviewed-pin-plan.json").write_text(json.dumps(report, indent=2) + "\n")
    for path, text in after_sources.items():
        require((repo / path).read_text() == before_sources[path], "source drift before pin update")
        (repo / path).write_text(text)

if __name__ == "__main__":
    main()
