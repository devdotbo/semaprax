# OPT808 source handoff

Status: source authored; no compiler, Cargo, builds, tests or runtime probes executed.
Audience: integration reviewers and focused qualification owners.

Issue: https://github.com/wavect/semaprax/issues/808
Base: `58ee1bcad7d0bf4b714ddcb76e527962f404ea17`.
Isolated branch: `codex/nested-response-string-808`.

## Source and controls

`NestedResponseV2` / `bounded-nested-response.v2` selects the same descriptor,
preflight arithmetic, UTF-8 lengths and functional escape writer. Its only new
read shape is a named borrowed `values:borrow Vec<string>` helper with
`vec_str_at(values,at)`. Version-specific names/IDs are selected before inserting
opaque authored identifiers. V1 remains independently callable with its original
primitive-vector refusal and all original tests intact. No input/output cap,
request one-vector/264-String census, compiler admission or ownership rule changes.

Three new lib definitions cover named slot reads, namespace opacity, old refusal,
two paths, exact bounds and unsupported shapes. Five new project definitions
cover complete primitive request→decision→response, independent JSON byte oracles,
empty/duplicates/Unicode/NUL/escapes, first/last, exact and plus-one selected/maximum
bounds, two paths, 131072/131073 escaped output and every-byte wire comparison,
canonical/HIR/graph/repeated C/plan/revision replay. All eight old project and two
old writer tests remain their own v1 routes. Backend definitions call interpreter,
native O0/O2 and Core Wasm; these are unexecuted definitions.

Two new native definitions reuse the complete original fault receiver,
read-only mapped input, unpublished fatal results and receiving Drop. They
measure actual allocation/copy calls for borrowed slot reads and complete/short/
byte/item preflight refusal, then the actual one Bytes + one final String and
one final payload copy. The complete original native test and its C receiver
remain unchanged. No fatal zero-leak/recoverability claim is made.

## CLI and version closure

Both command binaries share the exact new profile parser and keep every original
canonical/noncanonical bound refusal. Plan entries use the same parser and bind
v2 selector/identities through existing canonical receipts and replay. Suitability
v1 retains its contracted twelve-profile inventory; the new selector is explicit
in CLI help and the authoring card rather than silently widening that report.
Both scoped CLI help receivers expect the complete public help, including the
existing shape-discovery paragraph omitted by their base literals. Every previous
assertion remains. Quick-reference owning documentation assertions add v2 while
retaining the exact old refusal sentence.

The new version specification, v1 successor note, plan note, authoring/quick
reference and CLI-help note are linked through SUMMARY. The authoring card is
1664 bytes / 355 repository lexical units (caps 2048 / 512). All original fenced
quick-reference examples are byte-identical. Graft cards were read from the
indexed root checkout; the CLI is unavailable in this isolated checkout, so the
integration owner must refresh the graph after integration.

## Exact focused handoff

[selectors.json](selectors.json) contains 43 deduplicated exact rows, owning
source paths, package and target arguments. Every test name requires `-- --exact`;
no prefix or suite selection is proposed. Owners: semaprax lib, project, native,
semaprax bin, standalone CLI-help, documentation; semaprax-toolchain full bin and
full CLI-help. Old v1 source, backend, physical writer and complete refusal
receivers are retained in the list because the common emitter gained version
parameters. No generated pins were edited and no new producer is needed.
The diagnostic-help table/fences are unchanged; its ordinary equality gate is
included, with no handwritten JSON pin. Rust sources remain under 1500 lines.

[source-hashes.json](source-hashes.json) records exact changed/frozen paths.
Static checks only: Rustfmt parse/format, diff whitespace, old emitter literal
sequence resolved to v1, unchanged descriptor/templates/physical C receiver,
unchanged full fences, exact CLI-help literals and bounded source census.
These are not executable acceptance evidence, measured agent savings or issue
completion. Root qualification, push and issue decisions remain pending.

## Integration notes

Do not copy the root worktree wholesale. Root already integrated OPT722 and
PR809 independently. Merge the additive nested-response card and QuickRef hunks,
retain the current scaffold guidance and complete owned String-loop refusal
controls. CLI-HELP-V5 and SUMMARY are additive append/list merges; retain both
722 and 808 notes. No source audit body was relocated; both new test submodules
are explicitly attached to their existing owning harnesses.

## Descriptor-copy review correction at 0703

The native backend copies each primitive String pointer from checked Vec storage
with memcpy. The original v2 physical assertions incorrectly expected all copy
traffic to equal decoding plus only final payload materialization. The correction
preserves the backend, original total observer, exact input/output fixtures and
complete fault receiver. A probe-only forwarding layer identifies the exact
compiler String-slot extraction and verifies both its live descriptor owner and
a distinct live String owner. All forwards still reach the original total
memcpy observer once. Independent non-slot counters retain every other transfer;
complete independent wire bytes between distinct live owners identify final
payload materialization. No counter is subtracted, normalized or disabled.

A pointer-width eight-byte actual owned payload and separately owned descriptor
control perform the same-sized copies: the former remains a payload, the latter
a descriptor. Preflight/short/value refusal retains decoding non-slot bytes and
adds real descriptor bytes to TOTAL; successful encoding retains those descriptor
bytes plus exactly one complete wire transfer and two actual allocations. All
original fatal/recoverable fault, read-only input, publication and Drop controls
remain. Exact selectors are the same two native string_vectors tests; no new
binary or production change. Source-only: no compiler/build/test/probe executed.
