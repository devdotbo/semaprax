# Owned String Transfer v1

Status: source implementation and initial owning gates authored for OPT790.
Unexecuted; qualification and the complete acceptance matrix remain pending.
This contract does not expand public aggregate/UTF-8 ABIs or frozen profiles.

`string_take(value) -> string`, reserved `core.string.take`, consumes exactly
one live whole named owned String binding and returns the same owner. Source
String parameters use their ordinary implicit ownership spelling `text:string`.
A local or String leaf bound by recursive `match own` is a named binding.
Computed values, projected places, borrowed bindings, type arguments and
function instances are refused. Contracts cannot invoke take, including on a
contract-local owner. No implicit last-use move, replenishment or general
partial-move syntax is added. Ordinary owning reads retain clone semantics.

The current source and independent HIR receiver check the intrinsic signature,
operand type/mode/identity, availability and overlapping loans. Take retires the
source at argument staging; its CallCommit retires the argument epoch and its
result initialization owns the same carrier. The physical adapter forwards
that carrier: it neither transfers again from a dead epoch nor allocates a
replacement, copies payload bytes, converts UTF-8 or drops the forwarded owner.
Take itself has no language operation-failure or owner-admission status lane.
Its operand may still fail before staging; enclosing arguments, preconditions,
postconditions and constructors retain ordinary sticky failure and cleanup.
A staged owner is never revived after a later failure, and no failure publishes
its provisional result. Empty Strings remain live owners despite empty payload.

Cleanup construction and replay independently derive the moving operand. New
ordinary Take set entries prepay their maximum tree-node backing on the current
active compiler ledger before insertion; refusal is sticky and propagates
without allocating another message. Historical concat/format traversal scratch
and work remain an open ordinary inventory funding boundary. The
selected lexical replay and checked final-UTF8 closure retain an explicit
refusal, and the frozen candidate builtin catalog does not accept Take IDs. Compiler-owned callee identity
uses the closed static declaration variant; this spelling grants no admission
or loan authority. Existing capacities and source/type limits are unchanged.

The interpreter charges the operation/operand before an early one-operand
move, avoiding ordinary StringOp clone/values-vector staging. Native lowering
uses the existing canonical argument/result slots and live flags. Aggregate
and standalone private Wasm forward existing carriers/tokens and clear dead
argument bits without host copy/allocation/drop. Frozen scalar Wasm remains
closed to take, and the frozen compiler operation catalog is unchanged. Call
AST/HIR and canonical source syntax already represent the operation; there is
no new wire tag. Existing graph/cache/source replay must still resolve and
fully validate the current source, and closed catalogs do not acquire take
merely because `by_id` recognizes it.

Initial owning selectors (all unexecuted):

- lib `string_ops::take::tests::exact_named_transfer_has_one_commit_no_clone_or_failure_and_keeps_default_reads`
- lib `string_ops::take::tests::named_only_source_refuses_contracts_computed_borrowed_and_live_loan_transfers`
- lib `string_ops::take::tests::added_transfer_inventory_pays_its_node_exact_short_sticky_and_drop`
- language `owned_string_transfer::explicit_transfer_preserves_native_carriers_clone_baseline_and_failure_settlement`
- language `owned_string_transfer::explicit_transfer_interpreter_and_private_wasm_reentry_keep_selected_status`
- language `owned_string_transfer::named_destructured_string_leaf_keeps_clone_reference_and_all_engine_settlement`

The native allocation observer covers literal construction through settlement
for three repeated empty/nonempty calls and an unchanged clone reference,
counting realloc requests too. These source assertions are not measured gains
or all-backend no-copy evidence. The additional flat record owner compares
named destructured transfer with an unchanged owning-read reference on native,
interpreter and aggregate Wasm; only its native observer counts allocations. Further acceptance remains: recursive-leaf
and mixed String/Bytes record controls, first/late operand fuel refusal,
complete loan/status/transfer forgeries, complete compiler metadata funding beyond the new node
exact/one-short/sticky/Drop control, current Project cache cold/warm/provider drift and the
full original OPT779 forwarding populations. No Catalog rewrite, forecast
reduction, accepted-ticket completion or performance claim follows this draft.
