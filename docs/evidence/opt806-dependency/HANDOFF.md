# OPT806 separate refusal dependency handoff

Branch `codex/refusal-dependency-big-batch-539a` starts at frozen integrated
`539a5232c`. Prior recovery commits remain on their original branch. This source
batch removes annotation recovery's global `None` substitution; unknown
signature-type recovery retains its previously bounded behavior.

## Production behavior and preservation

`CheckedValue` and `Binding` are unchanged. Annotation-refused reads retain
actual type, mode, native-unit flag, availability, moved places, partial state,
contract freshness and loan/origin metadata. A separate borrowed refusal view
contains the exact annotation-root diagnostic ordinal and authored annotation
Type. It explains an expected-type mismatch only when the expected type matches
that view, or the owning operation accepts that annotation type. It never
supplies an inferred type or mode. Independent expected types such as
`let x: bool = 1; f_string(x)` still report their complete original `T205`.

Immutable direct aliases and transparent block tails retain that view. Mutable
bindings do not acquire views: mutation/branch flow needs a separate join model. Successful explicit
annotations and fixed declared call/constructor results reset it, so a later
unrelated result mismatch survives. Real moves, repeated-use errors, borrowed
transfer errors, active-view invalidation, native-unit/capability errors and
resource-mode guard/return obligations use full actual metadata. No source bounds,
frame layout constants, paid phase allowance or capacity forecasts changed.

Expression facts are sparse invocation-local BTreeMap nodes keyed by exact
borrowed AST identity. Keys are opaque pointers that are never dereferenced,
serialized or iterated to select diagnostics. Every production Enter clears
re-entry facts. Pointer reuse cannot occur for these still-borrowed immutable
program expressions. View lookup follows only a block's evaluated tail chain;
there are no function-body or sibling-statement scans. All extra storage is
explicitly unpaid ordinary residual storage, not a replay constructor claim.

This is complete for the bounded direct-expression obligations above. Richer
projection/member derivation, branch/match joins, numeric coercion-derived views,
generic result dependency, mutable binding flow and unknown signature carriers remain at their
existing physical inference behavior. A nominal annotation does not invent a
physical field or an owned carrier. Those broader cascades are not claimed fixed.

The production blast radius is recovery, Enter/blocks, operators, call arguments,
record/variant/update fields, match guards, closures, lexical/direct returns and
contract bool reception. Saved original receivers and their literal bodies stay
verbatim; they use inert Recovery and their original full receiving dispatch.
Both source audit owners include the new child. Original iterative/60-case
receivers and the complete seventeen-file archive remain protected.

## Authored controls, not executed evidence

The fifteen exact new libtest names are adjacent in `new-test-selectors.txt`.
Each takes `cargo test --locked -p semaprax --lib NAME -- --exact` in the root's
single combined focused batch. The original owning selectors in
`../opt806/HANDOFF.md` remain relevant; full gates are excluded by explicit user
instruction. No Cargo, build, compiler binary, test or Clippy ran in this lane.

The new child pins complete original/current diagnostic lists and original
Diagnostic objects for direct expected types, fixed result reset, unrelated and
successful explicit annotations, owners/borrowers, real loan invalidation,
scalar non-resource controls, scopes/lazy conditions, loops, returns, closures,
match coverage/guard modes, fields, contracts, native-unit and physical nominal
projections. It compares full original/current binding type/mode/availability,
place/move/partial/freshness/native/loan/origin facts, accepted canonical/graph
revisions and actual repeated receiver acquisition through Drop. The directory
control observes zero backing on successful reads and real error-path storage
release for 1/8/64/256 authored aliases. Expected lists are authored and pending
execution; no passing/current-head/public/runtime/token-cost claim is made.

Static qualification is rustfmt syntax parsing, git diff whitespace checking,
complete saved receiver/literal comparison, and a Rust source size census. These
are not substitutes for the authorized combined focused compiler run. Graft
remains unavailable in the lane; no graph refresh is claimed.

Complete current Rust source bytes and hashes are saved in `current-sources.json`;
`original-source-provenance.json` records the preserved original-body/literal
comparison. `module-size-census.json` records the static census of 5636 Rust
files, with zero violations and unchanged caps. These records contain no
compiler or libtest qualification.
