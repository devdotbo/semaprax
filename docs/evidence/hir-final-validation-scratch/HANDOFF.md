# HIR final-validation construction handoff

Status: source checkpoint; current-head owning execution qualification pending.

Audience: compiler contributors and reviewers.

Branch `codex/hir-final-validation-scratch-1a680` starts from exact integrated
`1a680e93e`. This batch is disjoint from monomorphize leaf substitution and nested
record root classifiers, and does not edit their owners. Root integration is
still required. No Cargo/build/compiler/tests/Clippy/full gates ran.

The bounded contract is `../../HIR-FINAL-VALIDATION-SCRATCH-V1.md`. Production
owners: `hir/nodes/import_result_type.rs`, `hir/nodes.rs`,
`hir/validation/callable_types.rs` and its inline frame child,
`hir/validation/nominal_facts.rs`, `hir/declaration_index/flags.rs`, and
`hir/declaration_index/flags/paid.rs`. The last owner authenticates the existing
payer with bytes(0) before fast results; its complete Storage<P> carrier and
original reservation initialize only in the non-inlined miss constructor.
The full old wrapper/classifier and unchanged compound miss payment remain.
No forecast, quota, ledger authority, binding/type identity, backend carrier,
canonical cleanup vector or work-count allowance changed.

Preserved full independent receiver bodies cover current pre-batch type
validation, eager type/ownership validation, original nominal directory
preparation, original paid classifier reception/miss construction/classifier,
and the complete ordinary native result type constructor. The entire main
validator, declaration-index classifier and function-value source are unchanged.
`source-provenance.py` verifies eight complete bodies and three unchanged full
sources against Git base; `source-provenance.json` saves all complete baseline
and sixteen complete affected current Rust source bytes and their hashes.
All 19 pre-existing classifier/nominal/replay test bodies also match exactly;
the source inventory test only gains the two new child includes. Baseline
textual copies are also adjacent, rather than in volatile storage.

All new children join the existing HIR and native interop source audits. The
native ledger owner uses its existing `source_inventory.rs` macro to append
complete child sources and the inventory itself, retaining every prior entry.
That owner is1497/1500. Static census covered5647Rust files: affected files fit,
with no introduced violation. One inherited violation exists at this base:
`tests/workspace/semantic_graph.rs`1501/1500, unchanged and not owned here.
No recorded cap was raised. Graft executable/index are absent; no refresh claim.

`new-test-selectors.txt` holds nine exact newly authored lib owners; every one
is pending execution. They cover complete all-kind native result carriers,
missing/forged compiler identities, exact hostile cached flags, invalid callable
signatures/arity, complete original/current type diagnostics, inline/spill order,
actual allocation acquisition through Drop, original/current paid metadata
entries, zero/exact/one-short/floor budgets and sticky same-owner retries. Existing
source tests and oracles remain complete, with no expected result weakened.
`existing-application-selectors.txt` and eleven supplied complete log copies
preserve actual58ee1bcad failures. Ten refused in final validation; selected05
refused in canonical cleanup plans. Authenticated stage receipts do not identify
an exact inner allocation, so no application-fit or byte-saving quantity is
claimed. Root's combined focused run must establish the current result.

Production risks are bounded scratch spill ordering, exact canonical type-key
comparison/cache precedence, native Result/Option name-to-ID matching, and payer
sticky authentication. The independent full bodies/carriers and hostile controls
address these directly. Real substituted type trees, classifier misses,
canonical cleanup-plan construction, type-reachability traversal and other
final-validator allocations remain; this is not complete physical funding of
the whole validator or removal of every G171 source.
