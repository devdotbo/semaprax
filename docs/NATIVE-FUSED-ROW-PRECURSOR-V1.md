# Native fused-row lease precursor v1

Status: private runtime prototype authored; owning executable qualification pending. No compiler profile, source admission or public provider route is enabled.

Audience: compiler, runtime, ownership and backend contributors and reviewers.

## Summary

This test-private C implementation advances the native executor prerequisite for
[finite construction fusion](OWNED-BYTE-CONSTRUCTION-FUSION-V1.md) and
[owned byte partition](OWNED-BYTE-PARTITION-V1.md). It supplies real bounded range
ownership, lifetime and cleanup operations. It does not emit an application,
accept a source candidate as authority, implement partition, or prove the
unchanged 4,096-row application passes. Interpreter and Wasm executors, descriptor
binding and complete native emitter integration remain pending.

## Held authority and representation

A caller-held issuer has a monotonically increasing nonce and no reset operation.
One active context borrows that issuer exclusively. Context activation, every
lease move and every borrowed view issue a fresh nonce; exhaustion refuses without
wrap. The module uses a distinct mutable static identity object's address.
The held issuer and module must outlive all children and borrowed views.

Private native64 Bytes carries three words: issuer identity, exact slot index
and nonce. Logical length is read only from the authenticated immutable slot. The index gives O(1) lookup while retaining full nonce checks.
This deliberately does not implement the proposed two-word representation: two
independent issuers may mint equal nonces. Exact issuer identity is checked before
slot lookup or payload access. Leased Slice carries six words: issuer, exact owner slot index, view nonce, owner
nonce, pointer and length. It cannot be cast to the ordinary two-word Slice.
Public pointers, provider carriers and foreign Bytes are unsupported.

The context contains 4,096 lease slots and 64 active view slots. The latter is an
explicit private prototype boundary, not generic source permission. Complete
metadata requires exactly `sizeof(spx_fused_context)` supplied bytes; adjacent
one-short preparation refuses before context initialization or backing allocation.
Production must precharge and allocate that metadata at the first Bytes operand,
after its preceding String preparation. A large stack context is not permitted
by this design and is not hidden behind existing frame limits.

## Construction, transfer and settlement

Count is positive and at most 4,096. Width is positive, with checked total extent
at most the unchanged 131,072-byte individual Bytes envelope. Lease issue follows
strict ordinal order; its range is ordinal times width with exactly width bytes.
A previous lease must be published before the next issue. The first issue
allocates and zeroes one backing after original row String preparation. The 32-event and cumulative 2 MiB byte budgets remain unchanged and
are debited before allocation with no cleanup refund. Administrative metadata is
separate physical backing, never disguised as a byte-work counter.

Moves retire the old nonce. Views pin the exact live owner and range; each has its
own nonce, and slot reuse retires earlier descriptors. Borrow begins only after Complete. Reads require authenticated
view identity and exact bounds before dereference. Move/drop with active views
refuses. Drop is permitted only in Complete or Aborted; Building requires abort
before canonical child cleanup. Complete requires every issued row published and releases the construction pin;
the last live child then releases the single allocation once through the explicit
callback. Abort retains the pin through canonical child cleanup; explicit
`settle_abort` releases it only after all children are gone. Children
survive sibling drops and administrative pin release. No per-child heap, global
arena, pointer-header inspection, overlapping windows or copy-on-write exists.

The private row push stages String and Bytes together, checking capacity before
retiring the row's byte token. Row swap transports existing affine handles without
copying ownership. The caller still supplies actual ordinary Vec storage and
canonical row leaf finalization; the prototype does not authenticate an arbitrary
supplied row pointer as a compiler-owned Vec. Actual allocator-backed Vec authority,
exact source push failure, ordering/comparison and generated cleanup hooks remain
required emitter seams. There is no replacement sorting algorithm in this tranche.

Clone, child partition, unknown producers, stores outside the independently
proved read-only suffix and public crossings stay closed. Sticky prototype refusal
is an internal observation, not an assigned language status/domain. Physical Bytes
allocation failure must retain the ordinary fatal boundary when eventually bound
to generated code; returning false from this private observer is not recoverable
application OOM support.

## Authored owning definition and remaining integration

`codegen::selected_fused_row::private_domain_affine_leases_fault_cleanup_and_reentry_at_o0_o2`
is registered only under `cfg(test)`. It compiles the real C implementation at O0
and O2 when explicitly selected. Definitions cover 4,096 disjoint three-byte leases,
exact scalar/String UTF-8 plus NUL payloads, one backing, complete settlement and repeated
invocations; metadata exact/one-short, extent/row bounds, nonce exhaustion,
foreign issuer, stale/length/pointer drift, overlapping slot drift, two views
and slot reuse, blocked move/drop and consuming swap, post-failure swap refusal, first allocation refusal with untouched output, and staged partial
cleanup at every cut of an eight-row construction. It has not executed.

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
currently authenticate the field name and a variable-shaped base; they do not
bind that source variable to the independently reconstructed lowered traversal
alias/index. Complete source-to-HIR authentication must close this explicit seam
before the mapping can become emitter authority. The mapping is private,
nonserialized proof data with no source admission or emission hook.

The native64 target mirror reports actual Bytes 24, View 48 and Row 40 byte
layouts, context 165,504 bytes and separately held issuer 32 bytes. C static
layout assertions bind these constants when the owning probe is compiled. The
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
