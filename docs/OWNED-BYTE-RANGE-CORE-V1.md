# Private held Bytes range core v1

Audience: compiler contributors and reviewers.

Status: authored executor-neutral prerequisite for OPT #746/#747; unqualified.
No source operation, allocation strategy, graph/cache tag, ABI or backend is
selected by this core. The unchanged4096 fixture remains refused.

`construction_fusion::domain::owner_core` holds one actual ordinary backing and
an exact fixed slot table. It accepts no child as a partition input. The caller
must independently authenticate the ordinary whole owner, target family/count
and normal no-live-loan transfer; the Rust owner type is not a source certificate.
The complete inventory publishes together only after width, checked extent,
actual backing length, existing Bytes extent and maximum Vec count checks,
metadata reservations and nonwrapping issuer preparation. Every precommit
refusal returns the original backing unchanged. Zero count requires empty
backing and positive width; it releases that backing once.

The context holds the actual issuer and module through Rust borrows or explicit
address-stable owned boxes. Tokens
carry invocation, slot, nonce and generation; every access validates them before
resolving a slice. Slot index determines the immutable disjoint range. A child
never gains whole-backing authority. Moves and stores renew nonce/generation;
active subrange loans prohibit them and owner drop. Retired view slots receive
new issuer nonces. The first operation failure remains sticky; exact loan-end
and owner-drop cleanup remain possible and cannot replace it.

The context must remain held while any child/container/view exists. A popped
child remains in its slot when its parent Vec and siblings settle. Last-child
settlement releases the backing exactly once. Context abandonment releases the
remaining administrative backing without publishing a result or synthesizing
language finalizer order. Executors still perform canonical per-owner cleanup.
The older descriptor-bound fusion reference now also releases abandoned backing.

Metadata predebits cover actual `Layout::array` slot/view/carrier backing,
context and returned/temporary Vec headers. Exact-capacity allocation is checked
before boxed tables publish; physical OOM is invariant fail-stop. The executor's
`Runtime::reserve_metadata` must debit its real held physical account, including
caller placement/overlap; this core creates no replacement budget or refund.
Deep clone calls the executor's ordinary cumulative event/payload debit before
fresh allocation and copies only the authenticated range. The executor must use
the existing32-event/2MiB counters and retain ordinary clone finalization.
No private parallel allowance is provided by the trait or descriptor.

Six child tests own original-owner preservation/exact and one-short metadata,
popped-child survival and deep-copy isolation, loan/store/sticky cleanup,
foreign/stale/overlap/exhaustion refusal, ordinary clone boundaries/partial
abandonment, and empty/invalid shapes. They are authored, not executed. Actual
interpreter/native/Wasm integration and all original partition/fusion owning
gates remain required by [partition](OWNED-BYTE-PARTITION-V1.md) and
[fusion](OWNED-BYTE-CONSTRUCTION-FUSION-V1.md).

The authored allocator-observer gate separates actual requested table/backing
bytes from conservative metadata header debits. It observes zero reallocations,
one original payload backing and three nonempty metadata tables at counts 1 and
4096 (only the view table at count zero), then zero live requested backing after
canonical children and context settlement. One-short preflight observes no table
allocation and preserves the exact original owner. This is an unexecuted
requested-allocation gate, not malloc-usable-size, RSS or target-backend evidence.

The private owned-holder constructor prepares the same core without a
self-reference or erased borrowed lifetime. Its invocation owner must retain the
context through every child and loan; the lifetime-free receipt alone grants no
escape or source authority. Both holder boxes are included in the real account
preflight before allocation, alongside all tables and headers. The authored
owned-context witness observes their actual requested layouts, context moves,
creator-scope return, last-child settlement and allocation-free one-short
refusal. This adapter enables the interpreter's held context implementation;
it does not select that executor or authenticate a source/HIR profile.

The interpreter precursor holds one boxed invocation context and affine Core
children in `interpreter/construction_fusion`. Its opaque carrier aliases cannot
mint another child. Byte reads use scoped callbacks; stores renew receipts;
vector publication is ordered after row operand evaluation. Block loan expiry
and canonical individual Bytes finalizers route back through the same live context.
Mixed aggregate settlement refuses until exact typed leaf order is joined; it
cannot sweep leased children ahead of ordinary String finalization.
Preparation owns the paid Runtime, preventing a different account from receiving
its metadata receipt. The enclosing State remainder plus the Core/header debit
cover the exact State once; payload, tables, temporary headers and stable
identity boxes are reserved before allocation.
Ordinary deep clone uses the existing evaluator event/payload counters and a
fresh ordinary backing. The private registry tests are protocol/account controls,
not selected-source authority or an original4096 execution qualification.
No production Plan installer exists in this precursor. Full sealed selected
validation and original interpreter row/Vec container allocation accounting
remain required before activation; legacy public/effect adapters cannot receive
an internal lease or recover its backing through ordinary Arc access.

The interpreter's loan reads take mutable access to the held core so its existing
sticky refusal selection can run. Comparisons resolve two authenticated loans
in one core callback, validating left then right without nesting mutable core
borrows or copying payloads. The owning paired-read regression retains exact
foreign-loan refusal, suppresses the callback on failure, and permits canonical
loan/owner cleanup without replacing that failure. This compile repair does not
admit a new source profile or qualify the original4096 fusion fixture.
