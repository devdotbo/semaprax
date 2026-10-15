# Native final UTF-8 allocation v1

Status: authored explicit native-only profile and owning regression definitions.
Audience: native ownership implementers and selected-profile adapter hosts.
No execution, allocation reduction, complete generated-codec qualification, or
full OPT #724/#737 completion is asserted. The default profile is unchanged.
The normative allocation cases come from
[Checked String Allocation v1](CHECKED-STRING-ALLOCATION-V1.md); ordinary
[cleanup and resource ABI](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) retain canonical settlement order.

## Selection and independent authority

`codegen::selected_final_utf8::emit_native_final_utf8` requires three distinct
persistent monomorphic identities: an encoder, a checked Report factory, and
an exact checked Report consumer. No manifest dependency, name prefix, private
kernel flag, signature-only match, or supplied sidecar selects this profile.
`replay_native_final_utf8` redoes ordinary HIR/cleanup/loan validation and every
profile check before comparing the supplied canonical certificate bytes.

The encoder is pure, non-yielding and outside the lexical-exit profile. It takes
`(borrow Report, usize)` and returns an exact two-case nominal outcome: one
String field and one usize refusal field. Complete declaration/case/field IDs,
Native64 layouts, constructor ExpressionId, ordinary OperationFailure producer,
function closure, canonical HIR graph digest, runtime asset digest, and capacity
policy are certificate data. The final String field is constructed directly by
`string_from_utf8(bytes_as_slice(owned_buffer))` in the result tail. No alias or
owning projected String read may obscure a clone. The bounded Bytes view uses
ordinary independently checked extent, loan and cleanup authority; its selected
payload ceiling remains 131072 bytes. Profile replay accepts the exact
compiler-owned `BorrowPlace` projection for `bytes_as_slice` after ordinary
HIR/loan validation, as well as the existing exact intrinsic-call projection.
An Array view or a named slice alias does not satisfy this owned-Bytes boundary.

The encoder closure rejects other String constructors/literals/formats,
String-containing owning parameters or helper results, cloning places,
String-containing record construction, maps, foreign/import/dynamic/generic
calls, effects, callbacks, resources and unexpanded materialization paths.
Borrowed Report/String views use existing admitted semantics. Helper names
carry no special authority. The final constructor has one ordinary propagated
OperationFailure source; the versioned sidecar additionally grants precisely
allocation.v1 codes 1/2 and convert.v1 code 1, class Adapter, retryability false.
These grant only this selected constructor, not default String/Bytes allocation.

The finite encoder and factory inventories are each bounded to 256 functions
and 4096 observations; the combined emission closure is at most 256 functions.
Profile replay charges at most 131072 work units, uses the existing cache depth
ceiling and a 32-level carrier walk, and bounds canonical sidecar bytes to 65536.
The original ordinary program is independently validated before constructing a
private emission view. Resource ABI preflight retains the complete original
program as validation authority; the view retains its exact types/interfaces
but deliberately has no ordinary main. The unchanged resource-lowering gate
still refuses resource-bearing contexts. All runtime discovery, layouts,
functions and adapters use only the selected view; original main calls or
literals grant no emission authority. The view is not exported as a new HIR or
cached graph. Original absent/forged entrypoints still fail full HIR validation.
Old graph/cache schemas and unselected generated C receive no new authority.

## Checked input provenance and public boundary

The factory has zero or one Copy usize parameter and returns the exact owning
Report type. Its entire pure monomorphic ordinary closure is replayed and emitted;
its default constructors and failure behavior remain default semantics. The
consumer is exactly `(own Report)->i64 {0}`, without contracts/effects/yields:
its ordinary canonical cleanup releases the Report's default owners. Neither
function receives selected String ownership. The selected encoder cannot be in
the factory closure. A foreign Report pointer is never accepted by the adapter.

`prepare_v1` reserves a generation before calling the retained allocator for one
exact `sizeof(Report)` backing, then executes that factory. Only a successful
source publication mints an opaque Report handle. Backing-allocation refusal is
an adapter transport refusal, with untouched out cells; it is not a fabricated
source allocation status. Source factory failure releases its backing after
ordinary canonical cleanup and reports the actual source status. Factory
String/Bytes OOM remains fail-stop and is outside this selected recovery claim.

`invoke_v1` accepts that matching live Report handle and an output limit. It
publishes either an opaque selected String handle or the exact ordinary refusal
length. Failure writes only the real context-local u32 status; the result cell
remains untouched. Report ownership stays live across invocation failure.
`release_report_v1` transfers it to the exact checked consumer, then releases
only its backing through the retained matching callback pair. Raw Report and
selected String results never cross an ordinary default-result adapter.

## Lease, reservation, failure and release

The host supplies one retained owner that physically owns the allocator state,
the exact initialized context/status arena, matching nonthrowing allocation and
release callbacks, and the generated native module. Its retention lasts through
all Report and selected output releases and through `close_v1`. The invocation
caller additionally pins executing module code until init/close returns. Lease,
context and status backing must not reside in an expiring invocation frame.
Callbacks return suitably aligned backing and may not mutate the context or
internal lease/handle storage. A callback's public-adapter reentry is refused by
the busy state. Host memory corruption is outside this checked FFI contract.

Eight fixed output slots and one Report slot reserve generations before external
allocation. Generations/borrow epochs cannot wrap; lease storage is one-shot and
cannot be reinitialized after close. Exact lease self-identity, a distinct-address module-local object minted at init,
and context generation/nonce reject copied, stale, cross-module and cross-lease
handles. Every public operation validates the issuing module object before
interpreting Report storage or invoking an encoder/disposer. An output borrow
has a distinct epoch; matching end-borrow is required before drop. Close refuses
any live Report, selected owner, borrow or invocation. Retained state is released
once only after all owners are settled. The selected header is exactly 16 bytes
plus payload and NUL; the last header word preserves ordinary borrowed length
views. This physical layout never authorizes default drop.

Before source execution/allocator callbacks, invocation records a real immutable
allocation.v1 code-1 u32 arena entry. At least two free arena entries are required:
one for this backing, one for the ordinary contract/conversion failure route.
A successful or ordinary-refused invocation may leave that unused authentic
entry in the arena; it is not selected failure and is never rewritten or recycled.
Allocator refusal selects the reserved token before initialization; overflow
selects code 2 before pointed input reads; malformed UTF-8 selects convert code 1
before allocation. A selected token is sticky within the invocation. Ordinary
canonical failure cleanup settles Bytes and any selected provisional owner;
postconditions and non-result cleanup precede successful handle publication.
Selected drop uses only the exact retained allocator/release pair once. Default
Report String/Vec releases continue to use their original allocator family.

## Owning executable definitions and remaining obligations

`codegen::selected_final_utf8::tests::` defines certificate/default-C parity,
forged/stale and hidden-allocation controls, and an actual compiler-generated
O0/O2 probe. The probe constructs its input through the checked source factory
outside final-allocation attempts, compares immutable independent UTF-8/escaped
JSON bytes, and covers exact/one-short limits, allocator refusal, malformed UTF-8,
postcondition failure, zero new owners on failure, final release to zero owners,
stale/cross/copied handles, borrow epochs, reentry, exhausted status backing,
transport refusal and overflow-before-read. Additional authored controls retain
exact original types/interfaces and empty resource ABI, refuse resource-bearing
contexts, exclude a distinctive original-main helper/literal from selected C,
and reject absent or forged original main metadata before filtered emission.
These are authored, not observed.

Production generated collection/nested-response composition and full corpus
qualification still require execution. Other direct String constructors, hidden
aggregate/default/argument clones, generic/dynamic closures, default Bytes OOM,
interpreter/Wasm injection, public language catch syntax, and other public ABI
escape families are not covered by this profile. They remain explicit #724/#737
obligations. This profile never changes their default fatal allocation contract.
