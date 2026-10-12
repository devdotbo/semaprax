# Owned String Transfer v1

Status: source implementation and 15 owning controls authored for OPT790.
Unexecuted; current-head qualification remains pending.
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
selected lexical replay independently reconstructs the exact named operand and
prepays its move-set node and key work. Checked final-UTF8 closure retains an explicit
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

Owning selectors (all unexecuted) cover `string_ops::take::tests`,
`interpreter::string_operations::take_tests`, retained receiving replay,
`language::owned_string_transfer`, recursive subtree transfer and current
Project cache drift. Discover bounded authoring guidance with
`help language author:string-take`.

The next acceptance tranche adds full empty/populated recursive String+Bytes
records, three repeated old-clone/explicit-take/subtree calls, pre/postcondition
and later-field failure, current ProjectFrontendCache cold/warm/provider drift
and restoration, canonical transfer/commit/status forgeries, and every fuel
limit below each measured source entry's ordinary completion. All are authored
and unexecuted. Fuel envelopes prove no partial publication or refuelling; they
do not independently count interpreter retained owner storage. The native
forward-to-score window separately counts allocation, payload copy and all
reallocation attempts; only the explicit-take/subtree controls require zero.
The original OPT779 recursive default-read zero-copy assertion is unchanged
and remains unresolved: its ordinary String read clones by contract. Aggregate
Wasm's existing strict host checks registry settlement; private internal Wasm
repeated results alone do not prove storage settlement. The independent last-view-use loan owner and actual operation/operand charge
boundary owner now cover forged loan metadata, exact pointer retention, unchanged
source availability on both fuel refusals, and complete owner/environment Drop
against the saved construction-only allocation baseline. The private internal Wasm owner now instruments the actual compiler-paired
host mint, byte-copy and authenticated drop boundaries for empty/nonempty,
clone-reference and failure calls. Exact literal request counts and balanced
owner/byte retirement supplement existing mandatory empty-registry settlement.
Those counters measure host payload operations, not total JS engine memory.
Historical ordinary inventory scratch funding and the broader
OPT781 transitive engine diagnostic boundary remain open. No issue closure follows this
source tranche.
