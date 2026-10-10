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
