# OPT806 source review preparation

Status: source checkpoint; current-head owning execution qualification pending.

Audience: compiler contributors and reviewers.

The static provenance script executed successfully in the integrated checkout
at `2adb721123ea9d9460fed12e2d3891e9bba1455e`, after the source cherry-pick. It
resolves the checkout from its own path and reads the exact baseline Git object;
it has no lane checkout path dependency. The later archive extension also passed
in the lane: ten complete receiving bodies, eleven unchanged body/file rows,
all sixty original proposal literals, and seventeen full original source files.
The archive extension must be cherry-picked before its integrated execution.
These are static comparisons only; no Cargo/compiler/build/tests ran.

## Specific production risks for review

- Recovery correctness depends on all eight lexical scope constructors resetting
  reused ordinals and the closure using a detached root. Adding a scope constructor
  without that hook can leak refused names into siblings or closures. The tests
  include sibling arms, nested blocks, loops, before/self reads and closures.
- Returning unknown changes downstream inference paths. The annotation/signature
  path must retain real ownership metadata whenever cleanup or transfer uses it.
  This batch limits annotation recovery to real by-value, cleanup-inert,
  non-native-unit bindings; owned and borrowed annotation errors intentionally
  retain baseline behavior. Their complete old/current diagnostics are pinned.
- Signature revalidation costs one declared-type check per affected parameter,
  and scans existing diagnostics for candidates. It does not scan whole function
  bodies per read, but this is ordinary unpaid scratch/directory storage and is
  not proof of a runtime improvement. Full Diagnostic equality plus binding type
  and mode prevent span-only poisoning; duplicate names and forged overlapping
  spans are qualification boundaries.
- A refused initializer becomes a recovery declaration only at continuation
  completion. Publishing earlier would hide self reads or independent nested
  initializer errors. The checkpoint tracks newly emitted errors and dependent
  reads; source span coincidence cannot publish a marker.
- H recovery applies only after original variant-pattern/name checks and only
  to missing payload declarations in that arm. Coverage `SPX-M101`, unrelated
  names, sibling arms and closure errors must remain independently selected.
- The full saved iterative receiver and original recursive oracle are separate
  preservation receivers. Their historic closure behavior differs, so closure
  tests compare full saved iterative/current paths; primitive oracle comparisons
  retain the original annotation-free domain. Arbitrarily widening oracle parity
  expectations could hide a baseline difference.
- No compiler execution has confirmed the newly authored expected lists, frame
  layout assertions, source-audit joins or compilation. The source risks above
  have authored controls, not passing evidence. Full gates remain excluded by
  the user's explicit instruction; only the combined focused checks are pending.

## Exact newly authored selectors

Each selector below is a complete test name, to pass with `-- --exact` in the
root's combined focused batch. This list was derived statically from the twenty
`#[test]` declarations rather than from compiler discovery.

```text
source_verify::recovery_tests::scoped_refused_let_retains_original_root_and_independent_reads
source_verify::recovery_tests::read_before_self_initializer_siblings_and_noncapturing_closures_remain_independent
source_verify::recovery_tests::existing_bindings_function_values_moves_and_match_coverage_keep_complete_old_verdicts
source_verify::recovery_tests::refused_operators_do_not_publish_synthetic_checked_types_and_keep_first_diagnostic
source_verify::recovery_tests::lazy_directory_reuses_ordinals_and_never_scans_authored_function_nodes
source_verify::recovery_tests::directory_and_full_receivers_observe_real_acquisition_and_receiving_drop
source_verify::recovery_tests::boundaries::noncapturing_and_nested_closure_refusals_remain_complete_original_errors
source_verify::recovery_tests::boundaries::existing_function_value_wins_over_a_missing_local_recovery_marker
source_verify::recovery_tests::boundaries::owned_and_borrowed_annotation_mismatches_preserve_real_transfer_verdicts
source_verify::recovery_tests::boundaries::valid_binding_and_sibling_scope_are_not_replaced_by_annotation_recovery
source_verify::recovery_tests::boundaries::refused_pattern_names_stay_in_their_own_arm_and_do_not_hide_coverage_or_closures
source_verify::recovery_tests::boundaries::repeated_source_uses_visit_only_active_scope_rows_and_release_actual_storage
source_verify::recovery_tests::boundaries::accepted_source_roundtrip_graph_revision_and_original_receiver_stay_equal
source_verify::recovery_tests::boundaries::initializer_completion_and_loop_ended_locals_keep_exact_root_or_independent_errors
source_verify::recovery_tests::boundaries::unknown_record_keeps_duplicate_fields_and_independent_initializer_errors
source_verify::recovery_tests::boundaries::unchanged_recursive_receiver_keeps_its_original_scalar_verdicts
source_verify::recovery_tests::boundaries::overlapping_signature_spans_cannot_poison_a_different_known_binding
source_verify::recovery_tests::proposal_corpus::full_proposal_corpus_keeps_exact_old_verdicts_and_scoped_intentional_deltas
source_verify::recovery_tests::proposal_corpus::refused_patterns_and_signatures_excuse_only_their_own_names
source_verify::recovery_tests::proposal_corpus::an_unknown_value_is_still_reported_when_no_refused_let_declares_it
```

For each line, the owning command shape is:

```sh
cargo test --locked -p semaprax --lib COMPLETE_SELECTOR -- --exact
```

Preserve the actual owning controls and original diagnostic/guidance producer
selectors listed in HANDOFF.md as part of that combined batch.
