# Owned Bytes finite construction fusion v1

Status: PROPOSED runtime contract for OPT [#747](https://github.com/wavect/semaprax/issues/747); source-only candidate inventory authored, owning executable qualification pending. No fusion route is admitted.

Audience: source, ownership, cleanup and runtime contributors and reviewers.

## Summary

An exact finite construction loop may eventually allocate one backing and issue
independent affine child leases while retaining the source's row construction
and publication order. The compiler must prove this strategy independently;
counting one source site as one physical allocation is not that proof.
[Partition v1](OWNED-BYTE-PARTITION-V1.md) and a complete selected Bytes owner
domain remain prerequisites. The original 4,096-row fixture and its acceptance
gates are unchanged and remain an open OPT #733 obligation.

## Authored source candidate inventory

`source_verify::declaration::fusion` currently derives a borrowed inventory for
one closed, nongeneric, effect-free source function and one explicitly identified
record containing exactly one i64, one String and one Bytes field. It accepts
arbitrary declaration/binding spellings. Names identify only exact lexical
bindings and types; filenames and known benchmark identities are not selectors.

The source shape has a fixed-capacity record Vec, induction initialized to zero,
condition `induction < count`, one immutable fresh record, atomic push into the
same Vec binding, and exact unit increment. Capacity equals the positive count.
The record's scalar uses that induction binding, String is a decoded literal,
and Bytes is direct `bytes_zeroed(width)` with positive literal width. Fields
must match declaration order and identity inventory; extra statements, aliases,
control exits, alternate constructors and intermediate observations refuse.

Count is bounded by the existing scalar/owned-carrier Vec envelopes for that
actual record shape. Checked multiplication derives payload and UTF-8 totals.
The proposed single backing must fit the unchanged individual Bytes extent;
this does not grant general multi-backing construction. The unchanged maximum
fixture derives 4,096 Bytes-producing executions, 12,288 Bytes payload bytes and
12,288 owned UTF-8 bytes. These are source inventory facts, not an observed
physical allocation count or a discount to the existing allocation quota.

A fixed 64-entry borrowed scope and existing depth/work bounds census every
supported remaining use. Vector moves invalidate the exact prior binding.
Immutable record traversal permits scalar reads and iteration-local String/Bytes
views; aliases expire with that lexical iteration. Supported remaining owner
operations are immutable Vec transfer, Vec length and consuming Vec sort.
Unknown/imported calls, explicit clones, additional constructors, owner mutation,
view escape and use after transfer refuse the candidate. Option byte observations
retain exact Some/None source shapes and scalar results. This is a source-only
closed use census; it cannot authenticate HIR nominal IDs or runtime leases.

On an existing `SPX-T267` allocation refusal, a successful inventory adds precise
unsupported-profile help. The error code, original message, legacy 32-event and
2 MiB limits, and all source/HIR/backend refusals remain unchanged. A candidate
is never converted into an admitted program, cleanup certificate or schema.
If the optional census scratch cannot be reserved, the help is omitted without
poisoning the already selected refusal; required compiler proof remains sticky.

## Required independent implementation before admission

Typed HIR must reconstruct the same count, induction, actual record/field IDs,
call signatures, current owner roots and closed use-domain from current source.
Missing, duplicate, foreign or stale facts refuse. Cached inventory, source
spelling and this advisory source result cannot substitute for typed proof.
Source and HIR must agree on each byte constructor and exact publication site.

The selected executor must implement the complete partition lease domain:
ordinary backing versus child authority, offset/extent/generation/module identity,
nonoverlap, stores and views, record/Vec transfers, sorting, iterator extraction,
normal/failure cleanup and charged deep clone. Children survive parent and sibling
drop. Child repartitioning stays closed. A source-owned String remains an ordinary
owned String with its full materialization and payload charges; sharing its
literal bytes is not permission to replace that owner or its drop behavior.
Unsupported providers/public ABI crossings must refuse before emission.

Backing allocation and lease issuance need explicit versioned accounting; no
legacy counter is suppressed. Retain 32 backing-producing events, cumulative
2 MiB with no drop refund, complete child metadata bounds and all existing Vec,
String, frame and builder envelopes. Authenticate actual required metadata and
reserve it before allocation, including spare/replacement/overlap backing.
No numeric status or schema identifier is assigned by this document.

Preallocation cannot silently move a failure ahead of an earlier source operand,
effect or String construction. An executor must either preserve each original
logical preparation/commit boundary, or independently prove the particular
reordering inert under the accepted failure model. Checked scalar failures,
allocation failure boundaries, early exits, partial rows and argument staging
retain left-to-right order, sticky status and canonical cleanup. Proving exact
trip count alone is insufficient to grant that reordering.

## Versioned allocation strategy capacity rule

`byte_data_capacity::AllocationStrategy` has two values, `Legacy` and
`FusedConstructionV1`. The strategy is an explicit argument to the selected
entry `analyze_with_strategy`. The existing `analyze` entry always passes
`Legacy`, so every current caller, report, diagnostic and graph projection is
unchanged. No flow node, cache record, graph fact, filename or stable id selects
a strategy. The returned summary records the strategy it was analyzed under.

`CapacityFlow::FusedConstruction { producer, rows, width, condition, body }` is
a distinct flow node. It does not reuse or relax the 32-capped
`FiniteOwnedLoop`. Under `Legacy` the node is an `SPX-H006` invariant, because
proof data alone grants no authority. Under `FusedConstructionV1` it requires:

- positive rows and width;
- rows at most 4,096, this versioned strategy's explicit row ceiling; source/HIR
  and each executor independently require actual record and owner layouts to
  fit their existing limits, without assuming a two-field or carrier size;
- a checked `rows * width` extent within the unchanged 131,072-byte per-value
  limit;
- `producer` naming exactly one unconditional, sequential body `BytesCopy`
  whose payload equals `width`.

The producer scan continues the ordinary flow walk's node count under its
existing node bound, refusing before the scan grows past it; it adds no
separate cap. A missing, renamed, resized, conditional, duplicated or nested
producer is an invariant. Row, extent and overflow refusals are `SPX-T267`. So is any other
allocation the body reaches (another producer, a call that allocates, or a
stdin read) and any allocation in the condition. Repeating a fused node inside
a finite iterator is an invariant. An ordinary `while` that reaches one keeps
its existing refusal.

A fused construction charges exactly one backing-producing event and its whole
extent. It never refunds a charge. That event and extent join the unchanged
32-event and 2,097,152-byte cumulative per-path limits, together with every
ordinary event on the same path: 31 ordinary events plus one fused event fit,
and 32 plus one refuse. `fused_backing_events` reports how many of a path's
`bytes_copy_sites` are fused backings. The rule leaves String quotas and costs,
failure selection, evaluation order, cleanup and public/provider closure to
their existing owners.

The rule only checks the flow it is given. It does not derive count, capacity,
extent, destination Vec, fresh row or no-escape facts. Those still require
independent source and typed-HIR derivation from current source. The executor
must authenticate and reserve its actual lease metadata before the backing is
allocated; this rule charges no metadata and assumes no slot layout. Only the
private, unvalidated source candidate route described below constructs the
node. No HIR selector constructs it, and no backend executes it. Owning
gates are `byte_data_capacity::fused_tests::` (nine definitions, unexecuted in
this tranche). The original fixture is not admitted, and the default check
still reports `SPX-T267`.

## Owning definitions and remaining gates

Four authored source-only definitions in
`source_verify::declaration::fusion::tests::` retain the original fixture, derive
other counts/widths and renamed bindings, and reject forged counts/increments,
constructor/field drift, escape, clones, unknown calls, shadowing and reads after
transfer. No definition has been executed in this tranche.

The original `maximum_admitted_owned_rows_preserve_views_order_and_nonallocating_reentry`
and both `hir::cache_codec::record_traversal_tests::` definitions remain unchanged.
Their complete same-source interpreter/native O0/O2/strict Core-Wasm and exact
source/graph/cache, byte/order/NUL/Unicode, first-read/no-allocation and complete
settlement/reentry observations remain required. Owning physical fault and
metadata exact/one-short gates must separately qualify the selected executor.
Source candidate recognition supplies no success evidence for those gates.

## Separate ordinary finite capacity prerequisite

[Finite owned-row construction v1](FINITE-OWNED-ROW-CONSTRUCTION-V1.md) is an
authored, independently source/HIR-derived prerequisite restricted to the
unchanged32 ordinary Bytes events. Owning executable qualification is pending.
It neither fuses backing allocation nor admits count33 or the original4096 loop.


## Private symbolic descriptor and reference domain

The private `construction_fusion` module independently reconstructs the closed
source candidate and symbolic typed constructor, retaining borrowed current
function, record/field, loop, push, latch and suffix identities. It is not a
serialized authority or a source-to-HIR equivalence certificate. Ordinary source
and HIR admission still charge every constructor and retain the 32-event limit.
Typed descriptor tests use admitted counts at most32; the synthetic4096 domain
probe tests dimensions only and does not claim a checked4096 program.

The reference domain holds an explicit module identity and non-resettable issuer.
Every owner/view carries issuer, invocation, slot, nonce, generation and range
identity. `new` preflights exact metadata dimensions without growing tables;
`start` takes the administrative construction pin. At the first original Bytes
operand, after the preceding String operand, the domain allocates the complete
lease table and independent64-view table, then invokes the backing allocator.
The executor must charge its actual layouts and backing capacities; metadata
allocation failure and Bytes allocation failure remain invariant fail-stop.
Each row must publish before the next issue. Completion releases the pin while
children survive. Abort retains it until canonical partial-row/Vec/String cleanup
has settled every child and `settle_abort` runs. Borrowing is allowed only after
completion; active borrows block move/drop, and readonly callbacks resolve exact
held backing ranges without cloning or exporting an owned backing. Writes are
closed in this first profile. Issuer or generation exhaustion refuses without
wrapping. The final child releases backing exactly once after the pin is gone.
Ordinary lexical/container cleanup cannot destroy the held invocation domain:
executor integration must retain it in the invocation registry until every
internal returned/popped child settles. Canonical abort cleanup still precedes
explicit `settle_abort`; Rust `Drop` is not a successful settlement certificate.
Whole-invocation abandonment releases the remaining administrative backing
without publishing a result or inventing source finalizer order. The private
range core and creator-scope returned-child control are described in
[held range core](OWNED-BYTE-RANGE-CORE-V1.md).

Two descriptor and five domain regression definitions are authored, including
exact/one-short metadata, independent concurrent view capacity, stale/foreign
identity, unpublished rows, abort pin order and issuer exhaustion. Callback counts
observe the reference protocol, not physical allocator measurements. Native,
interpreter and Core-Wasm integration, full source/HIR equivalence, physical
fault/metadata observations and atomic selected admission remain prerequisites.
No executable gate or allocation saving is claimed by this private tranche.


### Private Core-Wasm host-boundary precursor

`wasm::fused_row_host` is a test-only import/reexport probe, with namespace
`semaprax.private.fused-row-host.v1`. It is not a source emitter or a frozen
public Bytes ABI. Its host factory instantiates only the exact granted module;
imports stay private to that factory. The private carrier supplies a held issuer
`externref`, a slot and a never-reused nonce. Exact issuer object, invocation-held
slot/nonce and generation validation precede every payload read or release.
A different issuer, module grant, invocation or retired owner/view refuses. The
held issuer grants exactly one active domain, retained until settlement; overlapping
instantiation refuses, and sequential reentry retains the non-resettable nonce.

The host acquires fixed lease and64-view typed tables and one zeroed payload at
the first Bytes preparation. It retains the same publication, complete-only
borrow, abort pin and canonical settlement order as the reference domain. There
is no per-child backing, copy-on-write or read-time table growth. Its ordinary
32-event/2MiB counters are cumulative and do not refund on cleanup. Read views
are readonly; move/drop refuses active views. The owning probe uses a genuinely
checked eight-row descriptor and actual Core-Wasm imports, not4096 source
admission. It exercises foreign/stale handles, range boundaries,64 simultaneous
views, delayed abort release, last-child release, metadata exact/one-short and
reentry. Its signature/inventory and Node boundary definitions are authored but
unexecuted.

Reported table sizes are actual typed-array `byteLength` values. JavaScript
engine, closure, nonce and bookkeeping allocations are not a claimed complete
physical reservation or allocator measurement. Full executor storage accounting,
source lowering and immutable source/HIR provenance still have to join before
activation; this host namespace grants no production or ordinary admission.

## Source half of the selected FusedConstructionV1 proof

`source_verify::fused_construction` derives a borrowed, provisional source
proof for one function from the exact current AST. Its derivation is separate
from the diagnostic inventory above. The module must be closed, and no program
function may shadow a builtin the proof resolves. The function is explicit,
nongeneric, parameterless, effect-free and contract-free. Its body opens with
a fresh mutable `vec_with_capacity<Row>(N)` destination and a fresh mutable
zero induction, in either order, followed by `while i < N`. The loop body is
exactly an immutable row constructor, `vec_push` into the same destination, a
unit latch and a unit tail. The row record is explicit and nongeneric, with
between two and eight uniquely identified `i64`/String/Bytes fields, exactly
one Bytes field and at most two owned fields, matching the existing owned-leaf
Vec slice. Integer fields take the induction or a literal, and
String fields take literals; the proof reports their UTF-8 total, which is
still charged in full. The only selected producer is direct `bytes_zeroed`
with a positive literal width. Bounded, copying, stdin-reading or
helper-produced Bytes refuse, and no helper name is a selector.

The source constants are restated independently: 32 legacy rows, 4,096
selected rows and a 131,072-byte backing. The record's existing Vec envelopes
can only lower that bound: 8,192 rows overall, 8,192 divided by the scalar
field count, and 131,072 divided by 16 per owned field. Counts 1 through 32 derive `Legacy32` and keep the
unchanged `FiniteOwnedLoop` route. Counts 33 through 4,096 with
`rows * width <= 131072` derive `Selected`. A closed suffix census admits
immutable moves, `vec_sort_owned`, `for borrow` views and `vec_len`. It
records each move, sort and borrow statement as an ordered owner event and
refuses clones, unknown calls, owner mutation, shadowing, escaping views and
use after transfer. Each refusal is a typed step in source order.

The proof borrows its program, function, record, field, destination,
induction, loop, row, push, latch, producer and owner-event nodes.
`authenticate` re-derives the proof and requires the same nodes, and a clone
refuses. `producer_site` is the source capacity walk's `BytesCopy` site for
the producer. `raw_join_key` returns a freely constructible `RawJoinKey`.
It is comparison data, never authority, and it omits the owner events after
the loop and every current-owner fact. Before a join, the held proof must
authenticate over the actual AST and the whole program must pass source and
HIR validation. An independent typed-HIR proof must also cover the
construction, the closed suffix and the current owner at each transfer.
Only then may equal keys be compared.

These bounds are semantic charges, not physical sizes. The 131,072-byte
owned-carrier envelope charges 16 per owned field. It does not bound the
size of a row or the heap it uses. In native code a String owner is an
8-byte pointer (a borrowed str view is 16), so a selected row of Bytes,
`i64` and String is 24 + 8 + 8 = 40 bytes. An executor must derive the
actual target layout and fund its lease metadata separately. The existing
65,536-byte inline frame cap cannot hold a roughly 165 KB administrative
table on the stack. This proof raises no cap and no quota. The proof module
itself does not alter `SPX-T267` or default admission, and it does not reach
a backend. The source-only definitions in `fused_construction::tests::` are
authored but have not been executed.

### Private source candidate route

`source_verify::fused_construction::candidate::check_candidate(&Program)` is
the only consumer of a selected proof. It takes one parsed program and no
reader, path, library trust or other authority. It derives the single
selected proof from that exact AST, refuses when none or more than one
function selects, and re-authenticates the proof. Then it runs every
ordinary source pass through `declaration::verify_selected` with the held
selection. The selection is threaded explicitly; there is no global flag and
no skip switch. It changes exactly two places:

- The while-body byte-operation gate admits the proof's exact producer call
  by node identity. Every other ownership, loan, type, status and invariant
  check on that call and its loop still runs.
- `capacity::fused_selection` projects that loop as
  `CapacityFlow::FusedConstruction` from its actually projected condition and
  body, with the proof's `producer_site`, rows and width. It analyzes the
  program under `FusedConstructionV1` through the existing analyzer, so the
  loop charges one event and its extent against the unchanged 32-event and
  2 MiB limits. It then requires exactly one fused backing event in the
  selected function. Every other loop keeps its Legacy flow, including counts
  of 32 or fewer, which stay on `FiniteOwnedLoop`.

No count is rewritten and no limit is raised. A foreign, stale or
equal-text proof fails authentication: capacity reports an invariant, and the
while gate does not match it. The default `verify`/`check` path passes no
selection and is unchanged.

Success is a `ProvisionalFusedSource` whose only status is
`UnvalidatedUntilHirJoin`. It is not a checked program and grants no
permission to emit, cache, graph or publish. Before any validated program or
artifact, three steps remain: an independent typed-HIR derivation, an exact
join with the held source proof, and full selected HIR validation. The HIR
resolver's own 32-event while gate in `hir::resolve_statement` is unchanged
and still refuses the selected producer. The three executors must also
implement the lease domain before production enablement. Owning definitions
are `fused_construction::candidate::tests::` (three, unexecuted).

### Private validated selection factory

The HIR resolver's while gate is still closed on the ordinary path. Only the
experimental `hir::selected_fused_candidate::validate_selected(&Program)`
opens it, for the held producer node alone. It runs five steps in order:

1. `check_candidate` derives the source proof and runs every source pass.
2. The original source resolves with its real row count. A resolver
   `fused_producer` field admits only that exact producer pointer. Any type
   that declares invariants refuses, even one the row never uses. The check
   reads the borrowed declarations, because the ordinary route would resolve
   a clause program and then a desugared program with synthesized functions,
   and the held producer pointer would belong to neither. No clause program
   is built and no desugaring is skipped. Prior HIR is never reused.
3. The sibling `hir::fused_construction::join::join` derives the typed
   selection independently. It authenticates a freshly derived current source
   proof whose producer must be the held node, and joins the full prefix and
   suffix.
4. Full validation runs with a `Selected` built only from that `Joined`:
   agent, HIR validator, function value, identity, cleanup, cleanup-plan and
   loan-plan validation. The validator replaces only the joined loop's
   while-shape re-check. Capacity, both in the validator and at the
   resolver's capacity boundary, projects only that loop as
   `FusedConstruction` at the joined producer's expression identity. It
   refuses unless the walk matched exactly one loop and the selected function
   charged exactly one fused backing. Every other loop stays Legacy, under
   the unchanged 32-event and 2 MiB limits.
5. Only then is a `ValidatedFusedSelection` published.

The owner privately holds the `ResolvedProgram` and borrows the source. It
exposes the program read-only, the observed capacity facts, and
`with_joined`, which re-derives and re-joins on every call and lends the
result only to a callback. No constructor accepts HIR. Decoded, foreign and
stale programs fail the pointer checks. Ordinary `check`, `resolve` and
`validate` pass no selection and are unchanged. Nothing is written to a
graph, cache or artifact, and no route calls the factory by default.
Resolution and the factory each run the join, full validation and selected
capacity, so that work is done twice on purpose. It may only be removed
together with a sealed owner proof.

Owning definitions are `hir::selected_fused_candidate::tests::` (six,
unexecuted). They include cloned 4096-row HIR with tampered cleanup
inventory, cleanup plan, status sources and loan plan. Each tampered clone
still joins over the whole body and is then refused by full selected
validation.
