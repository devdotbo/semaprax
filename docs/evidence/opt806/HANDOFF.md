# OPT806 scoped recovery handoff

Source-authored batch on `codex/scoped-diagnostic-recovery-5728`, based on
`5728c9e48fe2afe5cd77e9edf000cbe171035bdb`. The containing commit is the source
candidate to qualify; this handoff does not claim executed compiler evidence.

The production source implements A/B operator refusal, C lexical absent-value
recovery, D return ownership diagnostic precedence, limited E cleanup-inert
annotation recovery, F authoritative per-invocation signature validity, G
unknown return/record refusal, and H arm-local refused variant payload names.
Original real binding/type/ownership/name checks run before recovery. Completed
initializer checkpoints, borrowed-name lexical rows, reused-scope reset and
detached closures prevent before/self/sibling/closure contamination. Independent
match coverage remains `SPX-M101`.

E intentionally leaves actual owned/borrowed values and all ownership-consuming
metadata unchanged. Separating refusal dependency from actual inferred ownership
and type metadata is a possible larger batch; returning unknown where ownership
consumes it would hide independent errors. H covers absent variant payload
bindings, not global suppression of coverage for any bad pattern. Ordinary
scratch/directory acquisitions are unpaid residual storage; no payer, phase
allowance, source bound or frame limit was invented.

## Preservation evidence

`source-provenance.py` performs static source comparison, not compiler execution.
Its adjacent JSON records ten complete legacy receiving bodies and four
unchanged original body/file rows against the exact baseline, with explicit
name/visibility/checkpoint/inert-state adaptations. It also checks all sixty
original PR804 source literals against `proposal-corpus-original.rs.txt`, copied
from `d36d47daef42e5d7e06e16a7ebade20be66ba95e`. Multiline literal contents are
compared, not dedented. Full original recursive and function receivers remain
independent. Both existing source-text audits join every new recovery/legacy
child. The original iterative verifier corpus is unchanged.

The authored corpus pins complete old code lists and separately intended current
lists, exact first diagnostics and original diagnostic subsequences. Boundary
controls retain real owned/borrowed transfers, valid binding/function precedence,
independent closure/match errors, overlapping signature spans, completed nested
initializers, canonical/graph controls, actual repeated receiver allocation and
Drop. Those assertions are pending execution, not passing evidence.

## One combined focused qualification batch

Run from the integrated candidate checkout with its own Cargo target directory.
These are selectors to include in the root's combined batch, not commands run in
this lane:

```sh
cargo test --locked -p semaprax --lib source_verify::recovery_tests
cargo test --locked -p semaprax --lib source_verify::iterative_verifier_tests
cargo test --locked -p semaprax --lib source_verify::constructor_tests
cargo test --locked -p semaprax --lib source_verify::declaration
cargo test --locked -p semaprax --test language variants_semantics
cargo test --locked -p semaprax --test language verifier_hints
cargo test --locked -p semaprax --test source_locked_contracts
cargo test --locked -p semaprax --test module_size
cargo test --locked -p semaprax --test projections installed_diagnostics
cargo test --locked -p semaprax-native-rust-interop --lib implementation::tests::ledger_capacity::hir_capacity_layout_constants_are_bound_to_root_const_assertions -- --exact
cargo clippy --locked -p semaprax --lib -- -D warnings
cargo test --locked -p semaprax --test documentation agent_quick_reference::diagnostic_help_is_pinned_and_covers_compiler_checked_examples -- --exact
```

If qualification demonstrates a guidance pin change, use its actual owning
producer and inspect the generated delta; do not manually relax a hash:

```sh
cargo test --locked -p semaprax --test documentation agent_quick_reference::regenerate_diagnostic_help -- --ignored --exact
```

The standard full quality profile remains required later by the owning gates.
No Cargo, build, test, compiler binary, Clippy or full gate ran during this lane,
per explicit user instruction. `rustfmt` and `git diff --check` completed; the
static provenance script completed. Graft executable/index were unavailable, so
no graph refresh was claimed. No public/hosted/runtime/token-cost qualification
is claimed. No root integration checkout was modified by this lane.
