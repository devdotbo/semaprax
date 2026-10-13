# Source-only inline audit worklists

Frozen source: `eb02a89b6174bced05e02f6ede2ebcda55cd5893`,
`src/hir/inspection.rs:40-905` plus its complete `audit_control_target`.
`frozen_walkers.txt` retains these original bytes. `old.rs` retains the full
independent program/attached/type/expression/shape receivers and their metadata
helpers. Its only traversal substitution is the independently saved allocating
type walker from `inspection/tests/type_identity/old.rs`; the copied control
helper uses an absolute module path. Wrappers expose complete receivers.

Production changes only worklist construction in the three iterative walkers
and the exact terminal NoDrop/Leaf returns. Push/extend/pop order and metadata
check order stay unchanged. The original recursive record-pattern walk remains
unchanged. Existing limits, type/index admission, and cleanup verification stay
with their original owners. The inline frame count is scratch capacity, never
an admission limit; overflow retains unbounded Vec backing with ordinary Drop.
This lane has no authenticated paid-spill owner and claims no funded spill,
whole HIR-stage payer closure, or final compiler capacity fit.

Pending exact library selectors (authored, unrun):

- `hir::inspection::tests::inline_worklists::compound_types_keep_full_old_diagnostics_and_unbounded_spill`
- `hir::inspection::tests::inline_worklists::shape_terminals_compounds_and_malformed_indices_keep_the_full_old_verdict`
- `hir::inspection::tests::inline_worklists::expression_roots_and_complete_public_audit_receivers_keep_old_results`
- `hir::inspection::tests::inline_worklists::inline_stack_keeps_new_children_ahead_of_older_spill_and_refills`
- `hir::inspection::tests::inline_worklists::complete_saved_receivers_are_bound_to_the_frozen_archive`
- `hir::inspection::tests::inline_worklists::complete_walk_bodies_keep_frozen_child_checks_and_push_order`

`tests/hir_module_boundaries.rs` joins the new scratch helper, controls, full old
receivers and complete frozen archive into the existing authority source audit.
No Cargo, compiler, build, test, native runtime or generated artifact operation
was performed for this source batch. Physical allocation assertions enclose the
complete audit invocation including Drop, but do not establish execution
results until selected against the integrated source.
