# Ordinary substitution and classifier leaf source batch

Status: source-only; executable qualification pending.
Base: `58ee1bcad`; branch `codex/substitution-leaves-58`.

Owners: monomorphize::substitute_type, substitute_source_function_type and
type_reachability::classify_nested_owned_byte_record. Owning RFC 0002/generic
record profile admission, cache semantics, diagnostics and completion gates
remain unchanged; no product row is promoted.

Resolved inline roots and empty nominal roots return without frames/results
carriers. A root TypeParameter retains exact owner-before-index diagnostics
and clones the same selected actual payload. Source inline roots and empty
Named roots likewise avoid traversal carriers; the first matching generic
name still takes precedence over nominal interpretation, including duplicate
names and missing actual arguments. Owned source names and compound substituted
payloads retain their original clones. Compound root traversals stay complete.

The nested-record classifier returns the exact original root result for
Bytes, Copy scalars and unsupported nonnominal roots before cloning or acquiring
its cursor. Nominal field/case traversal, limits, cycle detection and generic
admission remain unchanged. The existing ordinary flag leaf classifier already
has no cursor allocation and is not changed. Paid classifier/substitution
Storage and payer controls are intentionally retained unchanged, so no new
paid sticky-refusal or work protocol is inferred from these ordinary shortcuts.

Both full original substituters are preserved byte-for-byte in the test child.
The complete current classifier body is also preserved (only visibility in its
header changes), with an explicit reference to full saved substitution. The
independent original pre-reduction classifier remains intact and is compared
alongside the current saved body. No diagnostic, cap, allocation assertion or
hostile-input case is removed or weakened.

New authored selectors (not executed):

```text
hir::monomorphize::substitution_leaf_tests::resolved_leaf_substitution_keeps_full_old_values_diagnostics_payloads_and_drop
hir::monomorphize::substitution_leaf_tests::source_leaf_substitution_preserves_named_generic_priority_and_owned_payload_drop
hir::type_reachability::leaf_tests::classifier_leaf_roots_keep_all_original_profile_results_with_literal_zero_backing
hir::type_reachability::leaf_tests::real_nested_record_caller_preserves_full_old_limits_cycles_and_scalar_field_drop
```

Existing affected selectors:

```text
hir::type_reachability::classifier_tests::classifiers_preserve_saved_full_facts_limits_and_generic_refusals
hir::type_reachability::classifier_tests::classifier_query_allocation_reduction_settles_through_drop
```

Tests retain full old/new values, exact Diagnostic equality, foreign/missing
owner priority, every inline leaf, empty nominal and generic-name cases, large
owned callable payloads, and all final Drop. The real parsed record caller
exercises scalar substitution, generic refusal, recursion, depths through 65,
and field counts 0/1/256/257/4096/4097. Literal-zero leaf backing and actual
caller allocation reduction are authored assertions, not measured evidence.

The existing source_inventory macro child joins both new test/saved modules
with their production owners. The ledger root keeps its original 1494 lines;
existing original classifier source joins are preserved. All changed Rust
files remain below 1500 lines. Architecture ownership and schema are unchanged.

Source checks only: rustfmt with child traversal disabled, git diff --check,
and exact saved-body comparison against base Git source. No Cargo/rustc/build
or executable test was run. Graft returned no nodes and a cache permission
error; exact provided spans were used. RTK.md is absent in these checkouts.

The independent HIR deficit of 3477 bytes is a separate observed failure.
These local ordinary carrier reductions are not a demonstrated repair of that
deficit, whole-route funding, G171 or application-fit evidence.

Source SHA-256:

- `src/hir/monomorphize.rs`: `07bd9aa057c0a2d1b34fdbbd95d5cb948e6601922f8d5e9a404ebfd3c043acdd`
- `src/hir/monomorphize/substitution_saved.rs`: `919c77947a355d1f4aada31122775f1550ae0a814238335c06c30f3cbcacd0b3`
- `src/hir/monomorphize/substitution_leaf_tests.rs`: `755b0f94c26a4b55c3ee5058f797032a9f8bcd641e1b2dd6327a39c8a80cf555`
- `src/hir/type_reachability.rs`: `e4891df0f009ed6e749a77358cf913846ad352f8cc1bcb277ee909ba34ae04a7`
- `src/hir/type_reachability/leaf_saved.rs`: `507bbf24d5006217b5545c4fa2504346ed2e71b20d8c6d5a610a3934ae476b69`
- `src/hir/type_reachability/leaf_tests.rs`: `69bea6afcb8816e5315c77a7afb5b9755672f3edbc9db79d627bd37dbbc83628`
