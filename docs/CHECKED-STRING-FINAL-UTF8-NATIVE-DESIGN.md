# Checked final UTF-8 native integration design

Status: implementation design plus executable read-only compiler preflight.
No selected production emission, allocation-failure recovery, public native ABI,
interpreter or Wasm support is granted by this tranche. OPT #724 and #737 remain
open. The owning contract is [Checked String Allocation v1](CHECKED-STRING-ALLOCATION-V1.md).

Audience: compiler and native ownership implementers.

## First implemented boundary

`codegen::checked_final_utf8_preflight::inspect` independently invokes ordinary
full HIR validation before walking one exact monomorphic direct-call closure.
It records constructors, literals, String value/place routes, opaque callees and
dynamic/import boundaries in deterministic authored order, including contracts.
It has no ready/pass flag and cannot select an emitter. No generated function
name or callee prefix is accepted as authority. It bounds its optional report to
256 discovered functions, 4096 observations and 131072 charged walk work units;
the depth bound is the existing HIR/cache bound. Unknown or generic callees
remain visible and unexpanded, rather than silently becoming safe helpers.

These observations do not enumerate all implicit aggregate String clones or
prove a String Place is a move. Future selection must independently replay the
ordinary exact ownership/transfer/drop facts for every observation and every
String-containing carrier in signatures, locals, contracts and captures. An
empty report is never absence-of-materialization proof. The preflight does not
change default source admission, cleanup, graph/cache bytes or native output.

## Actual generated route and authority gap

The current collection/nested response writer constructs exact-sized Bytes,
fills it through ordinary checked helpers, then executes
`string_from_utf8(bytes_as_slice(finished))` and moves its String into the
public Encoded result. The final copy and String allocation remain real work.
The constructor has a canonical `PropagatedCall` OperationFailure source before
its result Initialize. That source authenticates the existing conversion call;
it does not, by itself, authenticate a new allocator domain or release callback.
The borrowed Bytes argument remains live on failure and is settled by the
existing canonical failure contour. Success initializes the temporary String,
then executes the existing exact result-transfer/publication contour.

The collection and nested response generators now place the constructor directly
in the Encoded String field. They do not route that owner through an ordinary
owning String Place read. This is an authored closed-constructor prerequisite,
not measured allocation reduction: the current native move classifier and the
actual generated physical witness remain the authority for default-profile work.
The existing byte-exact output, bounds, preflight and ownership gates still apply.

The raw public String/Encoded result currently leaves through a native boundary
whose release contract is the default String runtime. Replacing just its
constructor would let a callback-allocated owner escape to default free. This
is a concrete integration blocker, not a missing emitter switch.

## Selected profile certificate required before routing

Assign the admitting native profile and next selective proof/cache/graph schema
as one implementation. Keep all unselected profile bytes frozen. A certificate
must bind the checked closure and execution identities, HIR/source revision,
complete exact constructors and String-containing carrier inventory, allocator
contract, selected release dispatcher, and public boundary contract. Rebuild it
from source and independently replay it from HIR/cache; no trusted generated
name, function signature, private kernel macro or supplied certificate is a grant.

For the initial final-UTF8-only closure, reject every other direct or hidden
String allocation: literals, cloning places, aggregate clones, captures,
collection/map keys, defaults, contracts and external calls. Reject generic or
dynamic boundaries until their complete execution and materialization closure
is independently authenticated. Borrowed existing Strings and Report fields may
be read through their ordinary validated views; no new borrowing privilege is
created. Exact transfer facts must distinguish the final String move from a
clone. Primitive/record/variant result layouts and field identities remain
ordinary checked semantics.

The failure producer for each selected constructor must carry the exact
`semaprax.allocation.v1` codes 1/2 and existing convert.v1 code 1, class Adapter,
retryability false, and exact OperationFailure source before Initialize. Its
success/failure edges and cleanup state must replay independently. Do not mutate
an old PropagatedCall record downstream or infer domain authority from a callee
name. Selected allocator refusal is not a contract failure or runtime fuel.

## Allocator lease, owner and release contract

The host supplies an explicit allocator lease, matching nonthrowing release,
opaque allocator state and bounded allocation contract. The lease must stay live
until the last selected String is actually released, including a returned
Encoded carrier. An invocation-stack context pointer must never be stored in an
escaping owner. Either an explicit retained lease outlives that owner, or a
selected adapter owns both the result and its allocator context until release.
The public adapter must advertise and authenticate that matching release route;
otherwise selected owning results are refused at that boundary.

A selected header may preserve the length word immediately before payload so
ordinary borrowed length/UTF8 views coexist. Header layout does not authorize
default drop. Only independently selected cleanup may dispatch selected release;
ordinary/default owners cannot enter it, and selected owners cannot enter default
cleanup, argument-transfer or native result adapters. Release must use the exact
allocator/release pair that allocated the header, once, after the last borrow.
Header/callback metadata and retained lease storage count in capacity accounting.
No ambient allocator discovery, global mutable context or ownership inference
from payload pointers is permitted.

## Constructor phases and failure publication

1. Replay the selected closure, ownership, failure producer and release boundary
   before native preparation. Reserve real ordinary status-arena backing before
   any application allocator call; authenticate capacity and context lifetime.
2. Evaluate the borrowed input left to right. Keep its ordinary owner live and
   perform complete UTF8 validation before allocation. Check header/payload/NUL
   size arithmetic before pointed reads on an impossible length.
3. On invalid input or overflow select the immutable normalized record in the
   reserved arena and retain its real u32 token. On allocator refusal select
   allocation code 1. Selection is sticky and precedes result Initialize; later
   cleanup cannot replace it. Do not publish a domain code as a status token.
4. On success initialize one private header/payload/NUL allocation, copy exactly
   the input bytes and stage its selected owner. This is not final publication.
5. Execute canonical transfer, postconditions and non-result cleanup. Failure
   settles every initialized selected owner through its selected release. Only
   ordinary successful result commit may publish the selected carrier together
   with its authenticated lease/release boundary.

No change to GroupedCallCommit or Bytes ownership is implied. The final
constructor borrows the completed Bytes; its refusal does not consume them.
Physical Bytes OOM and every unsupported hidden String allocation retain their
existing contracts and must not be described as recoverable by this profile.

## Coherent next implementation sequence and owning regressions

First implement selected proof data/source and independent HIR replay, capacity
and graph/cache binding, plus a lifetime-bound selected native result adapter.
Then wire constructor allocation/status before Initialize and selected cleanup
release. An emitter-only or private-kernel-only switch is insufficient.

The owning native fixture must derive the real codec and exercise O0/O2:
empty/NUL/Unicode/control output equality, exact and one-short output preflight,
allocator refusal with untouched poisoned result/initialization state, real
status token distinct from code, sticky cleanup, zero owners after settlement,
re-entry, two simultaneous selected outputs and reverse release. Borrowed input
owners remain unchanged on success/refusal. Hostile cases must cover stale
constructor/closure/release binding, mixed fatal materialization, an escaping
invocation-stack allocator context, default adapter publication, wrong release
pair, early lease disposal and duplicate selected drop. Input construction stays
outside measured allocation phases. These are future execution obligations,
not results of the read-only preflight tests.

Interpreter/Wasm allocator injection, hidden aggregate/capture clones, all nine
constructors and application-level fault sweeps remain separate explicit #724
obligations. No cross-backend or full-ticket completion is claimed here.
