# Owned Bytes consume-and-partition v1

Status: PROPOSED, unimplemented contract for OPT #746. No route is admitted.

Audience: language, ownership, runtime and backend contributors and reviewers.

## Summary

Consume one existing Bytes value and publish a bounded Vec of disjoint owned
ranges over its backing. Administrative sharing never makes a source value
Copy, creates overlapping mutable owners, or grants public pointer authority.
This is a proposed allocation strategy, not an implementation or measurement.
The unchanged 4,096-row constructor loop remains a separate open obligation.

## Proposed source operation

```text
bytes_partition(buffer: own Bytes, count: usize, width: usize) -> Vec<Bytes>
```

The spelling and intrinsic identity require ordinary registry review; neither
is registered by this document. Evaluation and argument staging are left to
right. The operation consumes exactly one whole named Bytes owner at one
commit boundary. Its authority must independently identify an ordinary whole-
backing lease; a child range is not an eligible partition input, even when its
offset is zero or its extent happens to equal the backing extent. Existing loans
must have ended before staging its transfer.
There is no borrowed-input overload, projected-owner shortcut, overlapping
window operation, implicit clone, arbitrary pointer input, or padding/truncation.

Validate positive width, checked count times width, exact equality with the
input's authenticated logical extent, then the existing Vec<Bytes> capacity
and selected-domain metadata bounds, in that order. No guessed element count
or payload identity may replace these checks. Count zero is valid only for an
empty input and positive width; it returns an empty owned Vec. Every nonempty
child has extent width and offset index times width. Concatenating children
in index order reproduces every original byte, including NUL and non-UTF-8.

The Vec has length and logical capacity count. The existing element classifier
and carrier envelopes determine its maximum capacity; this proposal does not
choose a larger count because a fixture needs it. The implementation must
record the exact current Vec<Bytes> bound in its executable boundary tests.

## Transaction and failure selection

Before commit, authenticate the input lease and full backing extent, reserve
all required Vec, backing-administration and child-lease metadata, and prepare
the complete unpublished child inventory. The original owner remains the
caller-owned staged value until every semantic check and metadata reservation
succeeds. Failed preparation releases only unpublished temporary metadata;
ordinary canonical call-failure cleanup still owns the original Bytes.

At commit, invalidate the source lease generation and transfer its backing
ownership into the complete child inventory. Publish the Vec only after normal
result staging, postconditions and non-result cleanup. A failure cannot expose
a prefix Vec, initialize a caller result, leave hidden live leases, or replace
the first selected status. A zero-child success settles the empty source owner.

Proposed normalized domain: `semaprax.byte-partition.v1`. The following symbols
are proposed stable meanings, with numeric codes UNASSIGNED pending the existing
status/domain census and owning registry integration:

| Symbol | Selected condition |
| --- | --- |
| `width_zero` | width equals zero |
| `extent_overflow` | count times width exceeds semantic usize |
| `extent_mismatch` | product differs from authenticated input length |
| `metadata_capacity` | count or complete administrative inventory exceeds its bounded capacity |

Operand-evaluation failures keep their existing domains and left-to-right
precedence. These four operation failures occur before transfer. The eventual
mapping must be collision-free on interpreter, native and selected Core-Wasm;
no backend may invent a local code or reuse an unrelated arithmetic code.

Physical allocator exhaustion is distinct from semantic capacity refusal.
Under the current byte contract it remains invariant fail-stop. A recoverable
physical-metadata allocation domain would require a separately accepted checked
allocator contract, failure-before-publication implementation and physical
fault gates. Neither fuel exhaustion nor a logical counter proves physical OOM.
Forged/stale/foreign leases are boundary rejection or invariant failure, according
to the selected entry contract; they are never ordinary partition outcomes.

## Backing, range and generation ownership

Each live lease authenticates invocation/module domain, backing allocation
identity, offset, extent, generation and live state. Backing identity and lease
identity are different. One backing record owns the original allocation, exact
allocation extent and bounded live-lease accounting. Creating ranges retains
administrative backing ownership; it never copies payload bytes.

Live child ranges are pairwise disjoint. A renewed bytes_set owner retains the
same exact range and advances the appropriate lease generation. A borrow binds
the current lease generation and range; existing byte_get/byte_range rules cannot
read or write outside it. Child leases remain fully affine Bytes for the already
admitted normal operations,
including stores, views, transfers, clone and drop. V1 explicitly refuses
partitioning a child lease; recursive partitioning needs a separate contract and
metadata proof. No source parent or prior generation remains usable after commit.

Pop, iterator next and consuming traversal transfer the original child lease.
A popped child survives Vec drop, sibling drop and iterator abandonment. The
last live lease releases the backing exactly once; earlier child drops release
only their administrative lease. Clear and all early exit/failure cleanup follow
canonical element order. Administrative retain/release is not a source finalizer
or permission to reorder cleanup. Outstanding borrows forbid owner drop/move.

Cloning or nonconsuming owned copy-out deep-copies the selected range into fresh
ordinary backing and retains the existing allocation charges. Clone isolation
must be observable under later writes. No aliasing clone or descriptor copy may
stand in for that contract. Partial clone failure settles its prefix exactly
once while preserving the source lease and first selected failure.

## Quotas and metadata

The existing source-derived limits remain 32 allocation-producing Bytes events
and 2,097,152 cumulative payload bytes per executable path, with no drop refund.
The existing 131,072-byte individual Bytes extent, foreign input, Vec carrier,
frame and builder limits also remain unchanged. Partition issues leases; it does
not allocate new payload backing or erase the original constructor charge.
Deep copies and all other backing-producing operations retain their charges.
Runtime counters cannot replace independently checked source call-path bounds.

Lease metadata is a separate finite physical inventory, including slot spare
capacity, generation records, backing administration and Vec backing. A selected
invocation must precharge its complete inventory before allocation. A bounded
count alone does not prove total metadata size or repeated whole-backing partition
capacity; child repartitioning remains closed.
The executable contract must authenticate a finite slot/peak-live bound, reject
slot-generation exhaustion without ABA reuse, and test exact/one-short refusal.
No unbounded registry, global cache or per-child hidden heap allocation is allowed.

## Proposed native representation and public boundary

Only a new selected internal profile may use a two-word carrier:

```text
{ uint64_t opaque_lease; uint64_t length; }
```

The lease word is a generation-authenticated table handle, not a pointer. Every
operation receives or uses its explicitly held invocation/module owner domain
and resolves the handle before obtaining backing plus offset. Length is checked
against that record. No negative-offset header read, allocation-magic probe,
foreign-address dereference or guessed pointer provenance is permitted.
The exact handle bit layout and table size require their own target contract.

Selection must close over ALL reachable Bytes producers and consumers: literals
and constructors, copies/clones, partition, stores, views, record/variant/Vec
transfers, iterator operations, cleanup and imported calls. A carrier may never
reach ordinary free(ptr), a legacy helper or an unauthenticated foreign provider.
Unsupported crossings refuse before emission. Empty owners also need real live
lease identity and cannot borrow a sentinel as ownership authority.

The frozen public two-word pointer/length ABI is unchanged. A supported public
bridge must authenticate input, snapshot into selected backing, copy results out
through the existing host ownership contract and provider drop, and settle
private leases. It cannot export an interior pointer as a host-owned allocation.
Until that bridge has an owning contract and gates, public adapters explicitly
refuse selection. No general ABI or provider support is implied by internal C.

## Independent frontend and target proof obligations

Source admission must verify exact intrinsic/type/ownership, argument order,
call-path quotas, count/extent bounds and normal no-live-loan transfer rules.
HIR independently reconstructs the exact declaration and instance, operand
types, transfer commit, result Vec<Bytes> layout and range-domain selection.
Loans retain exact lease/range/generation provenance. Cleanup authenticates
source staging, provisional Vec, each initialized child and all failure paths;
attached plans, schema numbers, names or cached booleans grant no authority.

Graph/cache extensions require selective versioned tags through their owning
registries, exact source replay, missing/duplicate/foreign/overlap/stale range
controls and unchanged legacy bytes when unused. No tag/schema is reserved here.
Builder accounting includes full retained carriers and metadata spare capacity.

Implementation scopes can be disjoint: frontend owns registry/source/HIR/loan/
cleanup and graph/cache replay; interpreter owns backing-versus-range identity;
native owns the selected carrier/domain and complete producer/consumer closure;
Core-Wasm owns a distinct authenticated handle domain, immutable table bounds and
read/write fences; adapter owners decide public/profile support independently.
All scopes must join coherently before source admission. Existing public tokens,
private/public envelopes, import permissions and frozen target refusals remain.

## Required owning definitions, all pending

Use the existing owning harnesses; no definition or execution is supplied here.
Required same-source interpreter/native O0/O2/strict private Core-Wasm cases:
positive-width empty and nonempty partitions; exact/+1 extents and arithmetic;
byte sum/order/NUL/Unicode and non-UTF-8 preservation; mutation isolation;
pop/child surviving parent and siblings; deep-clone isolation and charges;
partial preparation/clone failure, postcondition failure, early exits, reentry,
sticky status, untouched failed outputs and exact-once final backing release.

Hostiles include stale generations, wrong domain/module, foreign handles,
overlapping forged ranges, child-partition refusal (including zero-offset and
whole-extent children), mismatched lengths, incomplete inventory and graph/
cache/source drift. Preserve 32 and +1 backing-event and 2 MiB adjacent controls;
physical native allocation interception must distinguish payload from metadata.
No allocation after first-read may be claimed until the actual owning trace
proves that particular fixture's read phase. Unsupported public routes must
refuse before producing artifacts. Owning executable qualification remains pending.

## Separate construction-fusion obligation

This primitive does NOT admit the unchanged 4,096-row fixture that executes
bytes_zeroed(3usize) inside its constructor loop. It cannot reinterpret 4,096
source allocations as one simply by naming a partition primitive. The separate
[construction-fusion contract](OWNED-BYTE-CONSTRUCTION-FUSION-V1.md) for
[issue #747](https://github.com/wavect/semaprax/issues/747)
requires a versioned general bulk-construction lowering with exact induction/trip
count, capacity, payload extent, fresh row, same Vec destination, no intermediate
owner escape, left-to-right effects and failure/early-exit equivalence, independently
replayed from current source. No filename, stable-id or benchmark constant can
substitute for that proof. The original maximum fixture and all payload/assertion
requirements remain unchanged. #733 and #746 completion and any efficiency
measurement remain pending.

Owning existing contracts: [byte data](PORTABLE-INDEXED-BYTE-DATA-V1.md),
[owned collections](OWNED-LEAF-COLLECTIONS-V1.md),
[cleanup](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md),
[call settlement](RFC-0004-NATIVE-CALL-SETTLEMENT.md) and
[public owned data](PUBLIC-OWNED-DATA-API-V1.md).
