# HIR borrowed generic reachability v1

Status: source-only; current-head executable qualification pending.

Audience: compiler contributors and reviewers.

Private source contract for one bounded lane. Qualification is pending: no
Cargo, build or test ran in the implementation lane, and no measured number is
claimed here.

`reachable_instances` (now in
`src/hir/validation/generic_template/reachability.rs`, re-exported by
`generic_template.rs`) no longer builds owned scratch copies. Its pending queue,
seen set and result borrow the exact `FunctionInstanceId`, `DeclarationId` and
complete original `&[ResolvedType]` argument carrier of the call node that
published them. The `HirValidator::reachable_function_instances` receiver returns
the same borrowed tuples, with a lifetime tied to the validated program rather
than to the temporary receiver borrow.

`visit_resolved_calls` and `try_visit_resolved_calls` gained one shared
expression lifetime so the callback may store the references it receives. Their
traversal bodies, preorder and clause order are unchanged.

## Preserved

* the 256 distinct-instance bound and its diagnostic;
* exact BFS first-seen order;
* identity authentication of every popped entry before seen suppression, so a
  forged duplicate is still refused;
* the first matching attached materialization for an instance;
* full-carrier equality in the parent materialized-sequence comparison;
* cycle termination, and tolerance of a missing materialization;
* requires, body, ensures order and all nested call preorder;
* exact `Diagnostic` values.

## Remaining owned boundary

`FunctionInstanceId::derive` still builds a fresh canonical identity for each
authenticated entry. That allocation is the authentication and is intentionally
not replaced by a hash or by trust in a cached ID. Diagnostics, derive identities
and the containers themselves remain allocated; only the cloned instance IDs,
declaration IDs and argument vectors are removed. This lane changes no
generic-template value environment, admission rule, function-meaning comparator,
observer, capacity or credit.

## Tests

Private unit modules under `reachability/tests.rs` keep the complete original
owned `reachable_instances` and `collect_calls` in `tests/old.rs` as an
independent reference. They compare full successes and full `Diagnostic` values
on forged carriers cloned from a checked minimal fixture, check by address that
published references are the original carriers, run the full `HirValidator`
receiver, and measure actual allocation through full Drop with the existing
observer. The 257 boundary is not allocation measured.

## Pending exact selectors

* `hir::validation::generic_template::reachability::tests::empty_and_every_argument_carrier_shape_match_original_with_exact_originals`
* `hir::validation::generic_template::reachability::tests::duplicates_keep_bfs_first_seen_order_and_first_original_carriers`
* `hir::validation::generic_template::reachability::tests::duplicate_identity_is_authenticated_before_seen_suppression`
* `hir::validation::generic_template::reachability::tests::distinct_instance_bound_is_256_and_counts_materialized_discoveries`
* `hir::validation::generic_template::reachability::tests::cycles_and_missing_materializations_terminate_in_original_order`
* `hir::validation::generic_template::reachability::tests::first_matching_materialization_wins_for_duplicate_ids`
* `hir::validation::generic_template::reachability::tests::requires_body_ensures_and_nested_call_preorder_is_exact`
* `hir::validation::generic_template::reachability::tests::full_validator_receiver_matches_original_and_outlives_its_borrow`
* `hir::validation::generic_template::reachability::tests::borrowed_closure_allocates_less_than_original_through_full_drop`
