# HIR borrowed aggregate metadata v1

The iterative HIR validator retains current declaration field/case slices and
current expression declaration IDs/type arguments across record construction,
variant construction, record replacement and variant-match frames. It avoids
cloning this immutable metadata solely to keep it alive while visiting children.
The validator's exact held Program outlives every frame; the receiver performs
the same current-index lookup and returns the same missing/foreign declarations
in the same order.

Field iteration, duplicate/coverage sets, child validation, type substitution,
ownership moves, loan checks and scope publication are unchanged. Genuinely
substituted types and result types remain owned. This is neither a cached
validation verdict nor a caller authority. It introduces no physical allocator
capability in the pure validator, quota change or static forecast discount.
The test-only frame capacity observation excludes only metadata that is no
longer owned by those frames.

The two owning lib selectors are
`hir::validation::aggregate_metadata::tests::borrowed_aggregate_frames_preserve_recursive_diagnostics_current_metadata_and_scope`
and
`hir::validation::aggregate_metadata::tests::borrowed_aggregate_metadata_removes_actual_owned_acquisition_payload_through_drop`.
The first compares complete errors, identity state and published scopes against
the unchanged recursive reference, including owned String fields, replacement,
variant construction/matching, concrete Option arguments and field/order/type
mutations. Existing direct-child, isolated branch and depth-512 late-error
oracles remain mandatory.

The second observes metadata acquisition through temporary Drop for 1/32/128
queries. Its independent saved operations clone the actual indexed Packet and
Choice metadata and actual checked Option type arguments. It proves the
narrow removed payload boundary, not whole-validator allocation totals or
Catalog/Shift memory fit. Qualification is pending until the owning checks run.
