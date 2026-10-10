# Checked String Allocation v1

Audience: coding agents, compiler contributors and reviewers.

Status: normative design and private native allocator-kernel precursor.
Production selection, source/HIR/cleanup integration and cross-backend
qualification are pending. A test-owned native status-arena adapter precursor
is authored separately from production emission. This does not complete [OPT #724](https://github.com/wavect/semaprax/issues/724).
The native gate is authored and unexecuted in this batch.

## Selection and compatibility

The proposed explicit profile identity is
`semaprax.checked-string-allocation.v1`. No manifest, public API, codec receipt,
source name, runtime allocator or generated artifact selects it today. A future
ordinary invocation selects it through an authenticated compiler/runtime
profile binding, carried through the checked closure, graph/cache and backend
preparation. A whole selected closure must implement this contract; mixing
checked and fatal String materialization within that closure is refused.
The exact admitting manifest and next cleanup/graph/cache schema numbers must
be assigned together with proof integration; this document does not mint those
schemas or silently reinterpret an existing cleanup plan.

Unselected profiles keep their existing source, graph, cache and target bytes.
The precursor C asset is included only by the native test harness; it is not
added to production emission, public ABI or runtime discovery. Existing String
OOM behavior remains outside recoverable invocation settlement until the
profile is fully wired. Existing normalized failure statuses retain their
identities and codes.

## Failure and result contract

Physical payload allocator refusal selects this normalized failure:

```text
schema:     semaprax.status.v1
domain_id: semaprax.allocation.v1
code:      1  (payload_allocation_refused)
class:     Adapter
retryable: false
```

Code 2 in that domain is `allocation_size_overflow`: the requested payload,
header or terminator cannot fit the target allocation size. A checked addition
refuses before allocation or pointed input reads. These are invocation failure
statuses, neither contract-false nor fuel exhaustion. Allocation retryability
is false for the current invocation; a later fresh invocation may succeed.
Invalid UTF-8 retains `semaprax.convert.v1` code 1 and performs no payload
allocation. Authenticated `borrow str`, String and char operands retain their
existing validity gates and add no borrowed lifetime privilege.

First failure is sticky. Every later checked constructor returns failure
without touching the result cell or invoking the allocator. Cleanup neither
clears nor replaces that failure. A refused result has no initialized owner
flag, result cell, provisional result, published carrier or staged output.
The status record must be created without allocating a new application String.
The final ABI integration must register the immutable normalized record in the
ordinary invocation context's reserved status arena and return its context-local
u32 token. A normalized domain code is never that ABI token.

The private kernel instead returns a Boolean success and stores the selected
normalized record in its explicit context. This is a deliberately narrow
precursor protocol, not the status/context/out-slot ABI. It stores a static
schema/domain/class and scalar code/retryability without allocation. Kernel
publication into its private result cell occurs only after complete construction;
the emitted caller still must stage that owner and run postconditions and
non-result cleanup before its ordinary final result commit.

## Allocation and ownership boundaries

The allocator is explicit invocation input: opaque caller state, an allocation
callback and its matching nonthrowing release callback. The kernel has no
ambient malloc, process, filesystem, network or allocator lookup. Callbacks
obey the selected bounded allocator contract; compiler/host/process exhaustion
outside this allocator is not covered. Allocator replacement cannot transfer a
live owner to an unrelated release callback.

Every constructor reserves exactly one header/payload/terminator allocation,
including the empty String. It initializes the private header and all payload
bytes before assigning the result cell. The header records byte length; NUL
is data, and the trailing terminator is never semantic length. Decimal rendering
uses a fixed stack buffer and unsigned magnitude, including i64 minimum.
No hidden formatting allocation occurs before the tracked payload allocation.

The kernel borrows its inputs and never frees them on success or failure:

| Kernel operation | Input and allocation contract |
| --- | --- |
| Literal | Authenticated UTF-8 bytes and exact byte length; one fresh owner |
| Clone | Borrow one live length-header String; one independent fresh owner |
| From char | Encode one admitted Unicode scalar in at most four stack bytes |
| From i64 / usize | Exact canonical decimal bytes; one fresh owner |
| From str | Authenticated bounded borrowed bytes, preserving embedded NUL |
| From UTF-8 | Complete strict scalar validation before one allocation |
| Concat | Borrow both exact input Strings; checked length sum, one allocation, two direct copies |
| Scalar collection String clone | Borrow one admitted String element; ordinary clone semantics |

These borrowed kernels do not alter source-level consuming concat. Its arguments
still stage left to right and transfer together at the declared canonical
`CallCommit`. A future checked consuming wrapper owns the committed input group
and uses its authenticated failure exit to settle it exactly once. Pre-commit
failure remains caller settlement. The caller cannot reacquire committed
arguments, and the kernel cannot privately free inputs before canonical cleanup.
Same-owner renewal retains its existing reservation/history rules. No backend
may infer cleanup from C pointer values or repair canonical vectors downstream.

Partial nested records, owned collection elements, clone prefixes and result
construction initialize each leaf only after its kernel succeeds. Failure
follows the live canonical plan's exact ordered finalizer contour. The scalar
collection clone kernel proves one leaf; it is not aggregate rollback or a
claim that record/Vec/StringMap/closure clone helpers are integrated.

## Proof integration obligations

All materialization points in the selected actual closure need an authenticated
failure source before initialization, including literals, implicit owned place
clones, conversion calls, concat, String slice/trim, map key clones, collection
and record clones, closure captures, default arguments and replicated contracts.
No source span, runtime generated name or helper presence supplies that authority.
The owning versioned cleanup/graph/cache schemas must be extended together.

Builder and independent HIR replay derive the same materialization inventory
separately. The new failure lane must be bound to the exact evaluated expression,
its failure domain, current region and live canonical state. Native lowering
may emit the new branch only from that validated proof. Failure reaches
`SelectFailure` and the authenticated `ReturnFailure`; success alone initializes
the temporary leaf. Removing a failure edge, initializing a refused result,
changing its status lane/domain, or borrowing another return domain must fail
`SPX-H006` independently. No builder regeneration is independent replay.

Native code must call the checked kernel, register its status, and branch before
owner initialization using that exact canonical exit. Interpreter must use
fallible host reservation and the same semantic materialization inventory;
its Nth-materialization injection is deterministic semantic evidence, not a
physical Rust/OS allocation claim. Aggregate Core Wasm needs a versioned checked
host import with status and an untouched result out-cell; the existing import
that immediately returns a carrier is insufficient. Its strict host and compiler
must authenticate status before local/carrier initialization. Runtime fault
injection is separate from ordinary semantic fuel and preserves its charge points.

## Authored narrow gate and remaining acceptance

`tests/native/checked_string_allocation.rs` compiles the actual private C kernel
at O0 and O2 with an explicit observed allocator. The gate is not ignored, and
missing Clang is a failure. It injects a real NULL callback result at the one
allocation in every direct constructor, then at all eleven allocations in a
mixed literal/clone/conversion/concat/collection-leaf chain. It compares exact
bytes including embedded NUL, Unicode and integer extrema; checks poisoned
result cells, unchanged borrowed inputs, first-failure status, exact reverse
release order, zero remaining allocations and successful reuse of the same
explicit allocator/context after a new invocation. Overflow and invalid UTF-8
refuse before allocation. This is private kernel evidence only, currently
unexecuted. Its caller-owned test cells are not a canonical CleanupPlan or
production compiler finalizer trace.

Required follow-up acceptance remains:

1. Authenticated opt-in selection plus complete source/HIR/graph/cache/cleanup
   failure coverage, native emitter routing and existing-profile byte pins.
2. Interpreter and strict aggregate Core-Wasm semantic injection/status parity,
   independent missing-edge/false-initialization/domain mutation regressions.
3. Native O0/O2 physical Nth failure at every reachable allocation in both the
   nested Unicode mixed-owner codec and an independent application shape,
   including late arguments, both sides of grouped commit, partial nested
   construction, provisional results, postconditions and reentry. Assert exact
   canonical traces, zero leaked owners, no duplicate drop or publication.
4. Explicit ordinary command-adapter mapping after canonical cleanup. Invocation
   allocation failure does not implicitly become the existing codec's typed
   `Error` or `Refused`. Any source-catchable allocation result carrier and its
   ownership protocol needs its own specified operation/admission contract.
5. Full #724 malformed/capacity/domain typed-outcome and composed same-source
   application gates and repository quality profile. No codec, OOM recovery,
   production-readiness or efficiency completion claim follows from this kernel.

Owners: [String Operations](STRING-OPS-V1.md),
[Native String Contents](NATIVE-STRING-CONTENTS-V1.md),
[Native Inline String Settlement](NATIVE-INLINE-STRING-SETTLEMENT-V1.md),
[RFC 0003](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md),
[Application JSON Codecs](APPLICATION-JSON-CODECS-V1.md).


## Private u32 adapter precursor

The test-owned compiler emitter `codegen::checked_string_allocation` emits the
existing ordinary native status runtime, the unchanged private kernel, and
`native_emit/checked_string_allocation/status_adapter.c`. Emission defines
`SPX_CHECKED_STRING_KERNEL_ONLY_V1`; this flag explicitly identifies a private
constructor-protocol probe. It is not `semaprax.checked-string-allocation.v1`,
a source/manifest selector, proof schema or permission to execute a checked
SEMAPRAX closure. The module is compiled only into the existing lib test harness.
Default production emission, String drop, graph/cache and cleanup bytes do not
change.

A fresh canonically zero adapter binds one ordinary initialized invocation
context and its caller-provided status arena, plus one explicit allocation/release
pair. Initialization refuses a full arena or an already initialized adapter;
there is no allocator-replacement API. The host must keep one arena slot available
until the selected failure is registered and keep its nonthrowing allocator pair
bound through every outstanding owner. Unexpected arena exhaustion remains an
invariant failure, not an invented language status.

Each of the nine borrowed constructors first checks the adapter's selected u32
token. It constructs into a private candidate cell; success alone assigns the
caller's out-cell. On kernel refusal it independently checks the closed record
set (allocation codes 1/2 or conversion code 1, Adapter, nonretryable), registers
that immutable record with `spx_status_record_adapter`, and retains the returned
context-local token. Later calls return that same token without allocating,
reading borrowed payloads or touching the out-cell. They do not append duplicate
records. A prior unrelated arena record makes the selected token differ from
its normalized domain code. This is selection within this private adapter;
it does not reconstruct the emitted function's ordinary sticky-failure local.

Owners still use `spx_checked_string_drop_v1` with that same explicit kernel
context. No ordinary `spx_string_drop`, ambient allocator, canonical cleanup
inference or allocator-compatible public String claim is added. The result
assignment is constructor success only; it is not the final function publication
that must follow postconditions and non-result cleanup.

Two authored lib tests own this precursor: deterministic emission/signature
inventory and a real C O0/O2 status-arena probe. The probe refuses NULL at each
of the nine constructor allocations, retains poisoned cells and borrowed input
bytes, checks immutable normalized records and distinct u32 tokens, verifies
reverse matching release/reentry, and checks overflow and malformed UTF-8 before
allocator calls. It also rejects full-arena and live-adapter reinitialization.
The observer counters remain cumulative across fresh invocations. These tests
are unexecuted and provide no current-head/native recovery qualification.

## Exact next native source-execution integration plan

The remaining boundary is semantic, not a runtime helper-name substitution:

1. Select the allocation profile in authenticated preparation input, bind it
   to exact checked closure/source bytes, and independently refuse mixed default
   and checked materialization. Assign the selective cleanup/graph/cache version
   together; this design does not reserve or guess the next schema number.
2. Derive source and HIR materialization sites independently: String literals,
   implicit owning-place clones, char/i64/usize/str/UTF-8 conversions, consuming
   concat, and scalar collection-leaf clones. Bind exact expression identity,
   lexical/return/contract domain, operation and live owner state. A runtime name,
   source span or kernel presence supplies no admission authority.
3. Add each site's allocation failure source before initialization. Ordinary
   literal and owning-place nodes currently initialize their temporary directly;
   they do not have an allocation `OperationFailure` exit. Reusing their success
   exit or another call's failure exit is forbidden. Source/HIR types alone do
   not authorize this branch.
4. Preserve left-to-right staging and `CallCommit`. Concat's kernel borrows its
   inputs, whereas its source call consumes a committed input group. The exact
   selected failure contour must own and settle that committed group; pre-commit
   refusal stays caller-owned. No kernel-side free or caller reacquisition may
   substitute for canonical cleanup.
5. Bind every selected String owner to the explicit allocator context in typed
   finalizer proof and all replay/graph/cache/backend projections. Ordinary
   `core.string.drop` currently emits `spx_string_drop`/free; it cannot release a
   custom-allocator checked owner. Either authenticate a new selective drop
   descriptor retaining that context through all moves, or independently prove
   one compatible allocator contract. A raw pointer/header resemblance is not
   proof. Context lifetime must outlive provisional results and every return
   postcondition domain.
6. Native lowering may call this adapter only after independent proof validation,
   branch through the site's exact canonical selected-failure exit before the
   owner flag is initialized, and initialize the success leaf only after status
   zero. Final result publication still follows ensures and non-result cleanup.
7. Cover every hidden String constructor in the selected closure before enabling
   it: slice/trim, map keys, aggregate/record/Vec clone prefixes, closure captures,
   defaults and replicated contracts. Until these have exact failure inventory
   and partial-owner cleanup, their actual reachability must refuse selection;
   a scalar-leaf constructor is not an aggregate rollback proof.
8. Authenticate the new status domain/codes in propagated-status replay and the
   ordinary command adapter after cleanup. Do not convert allocation failure into
   a codec `Error`/`Refused` carrier or add source catchability implicitly.
9. Author independent missing-edge, false-owner-initialization, wrong allocator,
   swapped failure-domain and cross-return-contour regressions; preserve old
   unselected source/graph/cache/native byte pins. Then run physical Nth-failure
   probes on actual generated closures, including late arguments, partial owner
   prefixes, provisional results and postconditions, before promotion.

Interpreter and versioned Core-Wasm status/out-cell integration remain separate
required parts of the complete profile. This native precursor neither supplies
those backends nor completes the composed codec/application obligations of #724.

## Closed final-UTF8 integration tranche

The [native final-UTF8 design](CHECKED-STRING-FINAL-UTF8-NATIVE-DESIGN.md)
records the exact generated-writer route and selected allocator-lifetime,
release-boundary and failure-proof obligations. Its production-compiled
read-only constructor preflight is not production selection or allocation/drop
authority. Existing native emission and String OOM semantics remain unchanged.
