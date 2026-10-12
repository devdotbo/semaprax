# Whole-subtree owned-record pattern binding v2

Status: source implementation; qualification pending. OPT779 remains open until
its source, HIR, cleanup, cache and three-backend acceptance gates pass.

This is an additive internal language rule over the existing bounded nested
owned-record profiles. It does not change Project/public aggregate ABIs,
generics, templates, variants, resources, stored views or collection families.
The v1 borrowed-pattern rule remains exact recursive destructuring.

An exact irrefutable `match own` of a named, unprojected owned record may bind
an entire monomorphic record field. For example `Outer { report, valid }`
moves `report` as one owned child record. Other siblings may use recursive
patterns, and authored fields may be reordered. Every declared field must
still occur exactly once; hiding an owned subtree with `_` is rejected.

Source verification and independent HIR validation authenticate the current
record, field, field type, ownership mode and exact generated `field.binding`
identity. The inherited depth64, transitive-owner256 and field4096 limits
count the complete root, including descendants hidden by the child binding.
A live descendant loan blocks the root move. Borrowed whole-owned-subtree
bindings, projected scrutinees and repeated moves remain refused.

Cleanup construction and replay derive independent declaration-order inventories.
One existing `Transfer` from the exact field prefix to the new binding root maps
all descendant leaf paths by their relative stable field-ID suffix. It preserves
actual completion history; it neither normalizes nor repairs canonical vectors.
The existing cleanup-plan.v8/graph.v28 vocabulary suffices: no field, transition
or wire representation changes. Higher lexical/loan schemas retain their
ordinary precedence. A v7 downgrade of a subtree destructure is refused.

The interpreter moves the existing child Record value. Native and Wasm create
ordinary aggregate carrier storage and move its fields, clearing staged owned
carriers; canonical leaf transfers and liveness flags still govern cleanup.
No owned payload or record tree is cloned, rebuilt or reacquired after failure.
Copy fields remain available in the destination. Arm failure and lexical exits
retain sticky status, canonical settlement and late result publication.

Owning controls include the independent recursive unpack/rebuild runtime oracle
in `nested_collection_record::subtree`, exact cleanup/graph prefix and mutation
checks in `nested_record_destructure_tests::subtree`, and existing nested-record
limit, loan, cache, source-identity and backend-hostile gates. Authored controls
are not execution evidence. Original Catalog23 forwarding is unchanged until
this complete acceptance boundary is qualified.

The additional `nested_collection_record::subtree::lifetime` owners keep a
transferred child live on actual Break/Continue paths, and instrument the native
forward-entry to first-consumer boundary for allocator requests and memcpy/
memmove payload work and every realloc request, alongside the old recursive control. The observer also
requires three exercised boundaries and complete canonical settlement at O0/O2;
it excludes compiler preparation. These authored controls are pending execution,
not a whole-backend memory or application-fit claim.

Independent ordinary replay now compares inventory entry places and supplemental
slots directly from current immutable HIR instead of collecting duplicate owned
expected directories. A paid sorted borrowed index retains the old last duplicate
storage-ID selection; liveness lookup still retains first duplicate flag selection.
A mismatch is selected only after the complete corresponding old fallible source
collection frontier. Supplemental traversal reuses one actual paid 1028-frame
cursor without changing its depth/capacity rule or replay Work equations.
Direct reconstructed shape vectors, lifecycle backing, type substitution vectors
and callable boxes acquire the current invocation's payer before construction.
The complete old coverage, supplemental and shape receivers remain independent
oracles. New original-corpus verdict/work, duplicate/late-error, whole allocation,
debit, exact/short/sticky and receiving Drop controls are authored source only.
Declaration fact-cache miss reconstruction, ordinary droppability classification,
and synthetic owning parameter-provider backing remain explicitly unpaid
residuals; no whole-replay funding, savings, OPT closure or execution is claimed.

The new owning selector prefix is
`cleanup_plan::replay::ordinary_inventory::tests::`, with controls
`complete_original_nested_and_resource_variant_receivers_preserve_verdicts_and_work`,
`late_unknown_entry_storage_precedes_retained_earlier_entry_mismatch`,
`one_paid_fixed_cursor_reuses_original_capacity_without_per_expression_requests`,
`whole_current_inventory_exact_short_and_sticky_refusal_settle_through_receiving_drop`,
`paid_substitution_preserves_complete_original_callable_and_nominal_payloads`,
`duplicate_inventory_ids_keep_the_complete_old_last_insertion_receiver`,
`complete_original_inventory_construction_receiving_and_drop_reports_both_real_costs`,
and `complete_paid_shape_receivers_keep_original_flags_exact_short_sticky_and_drop`.
