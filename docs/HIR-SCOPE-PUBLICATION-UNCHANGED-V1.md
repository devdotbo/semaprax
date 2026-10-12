# Unchanged HIR scope publication v1

Qualification pending. The private iterative validator still owns its initial
publication snapshot. On a changed snapshot it removes absent keys, updates
only bindings whose complete equality differs, and clones new keys and values.
Existing equal binding payloads retain their backing. It skips
`clone_from` only when complete ordered map equality proves the existing
snapshot already equals the supplied scope. `ValidationBinding` equality covers
contract-local/mutable flags, exact resolved type, ownership, availability,
moved projections, definitely partial projections and active loans. Keys and
all nested payloads remain part of equality; no fingerprint or partial summary
substitutes for them. Disabled publication and Drop are unchanged.

The full comparison visits current bindings and their nested equality payloads;
this is not a constant-time lookup or a claimed work/timing improvement. The
reduction is repeated allocation of unchanged owned snapshots. It creates no
new physical payer, authority, forecast discount or quota change. Initial
snapshot, changed binding clones, isolated branch scopes and their physical funding
remain outside this optimization.

The new lib owner is
`hir::scope_publication_tests::exact_unchanged_publication_avoids_recloning_wide_scopes_and_preserves_every_state_field`.
It compares the saved old publication, checks every binding state component,
key insertion/removal and disabled publication, and observes both complete
construction/publication/Drop lifetimes for 1 and 64 bindings and 128 attempts.
The independent literal old-clone census counts nominal declaration strings,
not guessed BTree backing. These are source controls, not executed evidence.

Unchanged mandatory late-error owners remain
`hir::iterative_validator_tests::validator_oracle_preserves_direct_child_scope_on_late_errors`,
`hir::iterative_validator_tests::validator_oracle_suppresses_failed_block_branch_lazy_and_match_child_scopes`,
and `hir::iterative_validator_tests::validator_oracle_handles_an_exact_depth_512_late_error_with_a_nonempty_scope`.

Removing the initial snapshot needs a separate scope mutation/undo contract.
Raw mutable map helpers currently take scopes and can fail after partial moves;
an untracked scope handle alone would expose uncommitted state on late error.
That larger journal is unimplemented here.

The changed-binding tranche adds
`hir::scope_publication_tests::changed::changed_publication_retains_full_state_without_recloning_unchanged_payloads_through_drop`
and
`hir::scope_publication_tests::changed::changed_publication_preserves_unchanged_backing_and_exact_delete_reinsert_disabled_drop`.
They retain the saved complete-map clone oracle and observe construction, initial
snapshot, 128 changes to one binding, final publication and all Drop for 1/64
bindings. Nested type arguments, move/partial paths and loans remain present.
Map deletion/reinsertion and disabled publication retain exact state. Existing
late-error owners above remain required. No execution is claimed here.

Synchronization performs ordered-map lookups and full binding comparisons;
this is an allocation optimization, not a claimed constant-time update or
linear-work improvement. No snapshot boundary, selected payer, default cap,
HIR wire format or publication authority changes.
