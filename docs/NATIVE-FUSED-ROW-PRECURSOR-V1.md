# Native fused-row lease precursor v1

Status: private runtime and selected native receiver authored. The production-shaped accepting API has no implementor yet; the only caller is a test-only UNVALIDATED precursor with no artifact authority. Owning executable qualification pending. No default compiler profile, source admission or public provider route is enabled.

Audience: compiler, runtime, ownership and backend contributors and reviewers.

## Summary

This private C runtime and its closed-domain native receiver advance the native
executor prerequisite for
[finite construction fusion](OWNED-BYTE-CONSTRUCTION-FUSION-V1.md) and
[owned byte partition](OWNED-BYTE-PARTITION-V1.md). They supply real bounded range
ownership, lifetime and cleanup operations and emit one selected function plus an
internal-linkage receiving entry. They do not implement partition, admit a source
program or prove the unchanged 4,096-row application passes: that source is still
refused by ordinary verification, so no typed program reaches the receiver, and
nothing here supports it. Interpreter and Wasm executors, the validated selected
owner and selected capacity/HIR admission remain pending.

## Held authority and representation

A caller-held issuer has a monotonically increasing nonce and no reset operation.
One active domain borrows that issuer exclusively. Admission requires headroom for
the invocation nonce and every row's lease before any debit; every move, store,
pop and view then mints a fresh nonce, and exhaustion refuses without wrap. Slot
generations are renewed by every transfer and refuse at their maximum rather than
wrapping. The module uses a distinct static identity object's address. The held
issuer and module must outlive all children and borrowed views.

The private native64 owner carrier is 24 bytes: issuer identity, exact slot index,
generation and nonce. A row stores the 16-byte slot/generation/exact-nonce form;
its domain authenticator is the row Vec header, which binds the held domain,
issuer and invocation nonce and is checked before any stored owner resolves. The
32-byte row equals the ordinary leaf stride, which the receiver checks against
the ordinary layout, so the ordinary Vec allocation boundary funds it unchanged.
Logical range is ordinal times width and is never stored or trusted from a
carrier. The 48-byte leased view carries issuer, slot, generation, view nonce,
owner nonce, pointer and length. None of these converts to the frozen 16-byte
public Bytes ABI, and no interior range ever reaches the ordinary tag-10
`free(ptr)` drop.

## Funding at original boundaries

Invocation admission runs before any program effect. It checks the issuer, checks
room in the one ordinary backing-event/payload account (unchanged 32 events and
cumulative 2 MiB), debits the separately held metadata ledger by the actual
`sizeof` heap frame plus the exact `count` lease table and 64-entry view table,
and holds one event and `rows * width` bytes. Only then is the frame allocated.
The frame holds the 160-byte domain header and every private carrier of the
selected function (staged rows, row Vec headers, leased views); none is on the C
stack, and no stack placement is described as a reservation. Every admission
refusal leaves allocator, String, issuer and debits untouched except the sticky
refusal mark of the refusing account.

The tables are allocated, and the hold is consumed into a debit, at the first
original Bytes operand after every earlier row operand; the backing follows.
Nothing there can refuse; physical allocation failure is fail-stop. Debits are
never refunded. A hold that no Bytes operand consumed, for example after an
ordinary Vec allocation failure, is released because it was never debited. The
metadata ledger's first refusal is sticky and it has no refund operation.

## Construction, transfer and settlement

Count is positive and at most 4,096. Width is positive, with checked total extent
at most the unchanged 131,072-byte individual Bytes envelope. Lease issue follows
strict ordinal order; its range is ordinal times width with exactly width bytes.
A previous lease must be published before the next issue.

Moves retire the old nonce. Views pin the exact live owner and range; each has its
own nonce, and slot reuse retires earlier descriptors. Borrow begins only after Complete. Reads require authenticated
view identity and exact bounds before dereference. Move/drop with active views
refuses. Drop is permitted only in Complete or Aborted; Building requires abort
before canonical child cleanup. Complete requires every issued row published and releases the construction pin;
the last live child then releases the single backing once. Abort retains the pin
through canonical child cleanup; explicit `settle_abort` releases it only after all
children are gone. Children survive sibling drops, Vec settlement and pin release.
The domain header lives in the admitted heap frame, outliving the creator function
and its parent Vec; invocation exit frees the exact tables, then the frame. A domain still holding
live children at exit is abandoned: backing and tables are released and no result
is published. No per-child heap, global arena, pointer-header inspection,
overlapping windows or copy-on-write exists.

The row Vec is allocator-backed storage bound to one held domain and generation.
Push checks ordinary capacity (Vec failure 1) while staged owners remain live,
then atomically commits the newest unpublished lease and renews its identity. Move
and consuming sort retire the prior header; sort compares fields in declaration
order with the ordinary stable insertion order and refuses, mutating nothing,
while any row is borrowed. Store replaces one row's child and settles the
displaced child; pop moves a row out while its child stays in the held domain.
Deep clone debits the ordinary event and width in the same payload account
before allocating a fresh ordinary copy of only the authenticated range. The Vec
finalizer visits rows in index order and fields in declaration order with the
ordinary String drop.

Child partition, unknown producers and public crossings stay closed. Admission
refusal is a private outcome, not an assigned language status/domain. Physical
backing allocation failure keeps the ordinary fatal boundary.

## Selected receiver

`codegen::selected_fused_row::emit_validated` is the explicit accepting API for
the full validated selected owner: an implementation of `ValidatedOwner` wrapping
the independent `SourceProof` joined by `hir::fused_construction::join` under
complete selected validation, with carrier facts rederived for that selection. No
type in this tree implements it. The receiver requires the owner to still hold
exactly the supplied programs. The old `construction_fusion` descriptor and
provenance reach the same receiver only as `UnvalidatedPrecursor`, which is
compiled for tests only and yields text with no artifact authority. Ordinary HIR
validation still runs as a necessary check, never as selection authority. The
default emitter never reaches either route.

Before lowering, the receiver refuses frozen public ABI crossings: parameters,
effects, contracts, a non-`i64` result, any Bytes value other than the single
proven producer, and any call, callable, host, import or exit outside the closed
operations. It checks a native64 host, the ordinary row stride, and binds its own
carrier layout mirror, emitting a static assertion for every carrier and the
frame. The retired single-object mirror in `construction_fusion` is not used.

The unit is the ordinary prelude, this runtime, the frame type, one function and
one internal-linkage entry; it has no `main` or exported symbol. The prefix binds
the exact selection sites: Vec allocation (ordinary Vec failure 3), induction,
then each row's operands in declaration order, issuing at the first original
Bytes operand. The suffix lowers only the census domain: scalar locals, checked
`i64`/`usize` arithmetic with ordinary arithmetic statuses, `i64_from_usize` with
the ordinary conversion status, lazy `&&`/`||`, `if`, blocks, loops, Vec move,
length and consuming sort, field reads (out of range is Vec failure 2), String
views, `str_as_bytes`, `str_len_bytes`, `byte_len` and `match byte_get(..)`. Each
view's representation comes from the producer-bound carrier fact for that exact
expression. Leased views end at their lexical block.

## Canonical cleanup order

The function's attached cleanup plan owns drop, staging and loan-settlement
order. The receiver admits the plan only if it can bind it exactly: no live
parameters, no lexical exits or resume bodies, no execution boundary finalizer,
operation-failure lanes only, each status source reaching its own
`ReturnFailure` exit through blocks that only select that status, no variant
inventories, and one scalar result commit. Liveness flags follow the canonical
executor's rules at each transition's exact expression. Every ordinary failure
jumps to the plan's own exit for that status source; the exit ends live loans,
aborts a building domain, runs that exit's `finalize_in_order` verbatim against
the carrier registered for each plan storage, releases the pin and returns the
first status unchanged. Region normal ends and the success exit are emitted the
same way; publication follows non-result cleanup and a dead-flag check. Nothing
is sorted, unioned or repaired. A plan transition never reached, a plan failure
the lowering does not preserve, an ordinary failure without a plan source, or a
finalizing exit without a position refuses the shape. Domain authority failures
inside the function are invariant fail-stops; it has no refusal edge.

The entry publishes only after the domain settles and the postconditions hold:
the exact metadata debit, exactly one ordinary event and extent consumed from the
hold, and no sticky failure. Its private outcomes are published, language
failure, admission refusal and abandonment.

## Authored owning definitions and remaining integration

`codegen::selected_fused_row::private_domain_affine_leases_fault_cleanup_and_reentry_at_o0_o2`
compiles the runtime with a test-owned allocation census at O0 and O2. It covers
4,096 three-byte leases with one 12,288-byte backing, sum 8,386,560, admission
funding before the frame and tables after the first String, stable sort, reads,
settlement and reentry; one-short metadata, full payload account, overlap, frame
placement, released unconsumed hold and issuer exhaustion before any allocation;
fail-stop after funding for every physical allocation; ordinary Vec allocation
failure; staged partial cleanup at every cut of eight rows; foreign, stale,
generation, view and compact-row header drift; loan-blocked sort and drop; pop
survival, store, charged clone at the event cap and generation exhaustion. No
probe resets a sticky failure or rewrites a ledger counter.

`codegen::selected_fused_row::tests::` add receiver definitions for counts 1, 8
and 32 through the unvalidated precursor: plan-bound lowering with verbatim
failure exits and frame-held carriers, unchanged default emission, stale, foreign,
decoded and drifted precursor refusal, census refusals, accepting-API refusal of a
stale or foreign owner, cleanup-proof refusals, the layout mirror, and
executable units at O0/O2 for the oracle with allocation order and reentry,
admission refusals, ordinary Vec failure, call depth, suffix failure with a live
loan and fail-stop. The original 4,096-row definition is ignored until the
validated owner and selected admission exist. None of these has executed.

Emitter binding must independently match the actual immutable typed descriptor,
constructor site, row field identities and complete suffix use-domain. The original
suffix mixes leased Bytes views with String-derived ordinary Slice values. Carrier
choice therefore needs exact producer and alias provenance, never ResolvedType
alone; mixed phi/call/return and unknown producers must refuse until specified.
Enlarged local/view/metadata and record/Vec layouts must be physically reserved;
the legacy record layout cannot be reinterpreted as this larger private carrier. Same-source
interpreter/native/Core-Wasm, current graph/cache, ordinary public byte parity and
original full application gates remain required before any admission activation.


## Private mixed-view provenance prerequisite

`construction_fusion::native_provenance` independently derives the current
source/HIR descriptor, then walks the retained suffix with a precharged fixed
cursor and lexical alias table. It records exact expression pointers and IDs,
actual field declaration and vector root, and the original field-read producer.
String fields and their `str_as_bytes` aliases retain ordinary borrowed carriers;
Bytes fields retain leased carriers. Equal `Slice<u8>` types alone never select
an executor representation. Source spans are matched to current source expression
kind, field/alias name and call shape as additional binding checks. Unknown,
foreign, mixed-result or escaping view producers refuse. Source field checks
now require the exact authored traversal-body pointer and source row alias.
A separately reconstructed frozen HIR region binds its owner place and row-index
identity to the unique current source `ForBorrow`, vector alias and index name.
The canonical synthetic snapshot span remains the source item span, matching the
actual lowering contract. Current owner/index drift, source alias mismatch,
ambiguous regions and foreign body pointers refuse. The mapping is private,
nonserialized proof data with no source admission or emission hook.

The native64 target mirror reports actual Bytes 24, View 48 and Row 40 byte
layouts, context 165,504 bytes and separately held issuer 32 bytes. These describe
the retired single-object precursor context. The selected receiver does not use
this mirror; it binds its own in `codegen::selected_fused_row::layout` (160-byte
header, heap tables, 32-byte stored row, 56-byte row Vec header). The
4,096-row arithmetic reaches the unchanged 131,072 owned-leaf byte envelope;
full row storage includes its independently counted scalar bytes. Every view
expression plus potential immutable alias receives a conservative two-slot,
48-byte-per-slot frame upper bound. These are preparation requirements, not
executed allocations or permission to hide context metadata in a checked frame.
Actual selected lowering must charge these representations, metadata and
replacement overlap before allocation at the original preparation boundary.

Three authored `construction_fusion::native_provenance::tests::` bodies cover
mixed origins and exact aliases, current producer/source drift, and complete
proof scratch/fact backing exact and adjacent one-short budgets. Checked ordinary
counts 1, 8 and 32 supply the HIR; the original 4,096 source remains T267-refused.
These definitions and the updated C probe remain unexecuted. Complete lifetime
release integration, producer-bound lowering, allocator-backed Vec authority,
interpreter/Wasm equivalence and owning executable qualification remain pending.
