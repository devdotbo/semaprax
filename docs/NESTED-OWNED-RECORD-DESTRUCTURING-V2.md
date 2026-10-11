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
