# Retained expression-fact lookup v1

Status: SOURCE ONLY; private candidate, executable qualification pending.
Audience: compiler contributors and source-accounting reviewers.

## Boundary

This candidate implements only the dense certified ordinary status-source loop.
It does not retain search state into block/edge validation or across invocations.
The owning issue is #885. No runtime gain, application admission, public API,
hosted support or completion-matrix promotion is established by source analysis.
The existing 67,108,864-byte application cap is unchanged.

The receiver first executes its original directory construction, held-prefix
acquisition, ordinal initialization, first insertion, and fact receiving payment.
Only at the first actual old lookup frontier does it authenticate zero bytes and
work, pay new selection controls, and consider retaining scratch. A query-free
route or error before that frontier acquires no new owner or selector.

Eligible owners are nonempty immutable Arena facts under ActiveBuilder, with a
status count at least the target-layout `MIN_ROWS`. That constant conservatively
requires a positive source fee difference on full successful traversal; actual
query count can be smaller on an early error. Short, empty and LegacyUnobserved
routes keep their old lookup algorithm after the paid first-frontier selector.
SelectedReservation, Tree, fewer-than-three status rows, and edge lookups keep
their original dispatch. Selector overhead on ineligible held-prefix routes is
a real cold-path cost, not a saving.

## Custody

The session holds immutable arena and function loans and an exclusive mutable
ReplayBudget loan. Its query identity loan must last for that entire session:
production queries come from the held function's immutable status table. The
actual slice-bearing comparison frame persists; it is neither a paid-once flag
nor a recreated full local comparison. Every field is reset on each query.
Returned facts borrow the original arena and never mutable scratch. There is no
allocation, RefCell, borrow guard, memoized verdict, pointer-only budget seal,
refund, future-query guard bank, or fallible scratch Drop.

Each row preserves `HeldPrefix::insert -> fact receiving payment -> lookup`.
The existing insertion owner authenticates the exact directory, function,
ReplayBudget and current ClassifierLedger before its existing payments. Its
complete original fees remain in force. The first insertion precedes owner
acquisition; every later insertion occurs inside the same closed continuation.
No callback or ACTIVE-switching operation exists between that guard and lookup.
Thus the old zero-byte/work lookup authentication still addresses the same held
invocation. This is source-bound lexical custody, not permission to expose the
session to arbitrary callers or resume it under another invocation. Reentry,
foreign function/budget/arena replacement and source-lifetime escape are not
admitted by the borrowed session interface. The existing enclosing proof and
insertion guards continue rejecting foreign/stale/nested invocations.

Changing the continuation's callee set or allowing a callback requires a fresh
custody audit; an unpriced dynamic preprobe cannot replace this closed scope.

## Funding

All formulas use actual target `size_of` expressions in the owning Rust modules.
There are no numeric host-layout assumptions in admission.

- `S = sizeof(LookupControl) + 2*sizeof(old ReceivingComparison)` is the exact
  immediate predecessor's reusable scratch, exported by the unchanged prefix
  implementation solely for source accounting.
- `A = SELECT_BYTES + Session::acquisition_bytes()` includes selector scalars,
  first-authentication/payment/helper Result overlap, helper actual/formal
  transports, full initialized/receiving Session and nested field initializers,
  constructor arguments, and acquisition reservation/work Result controls.
- `R = Session::query_bytes()` includes new mutable session actual/formal
  borrowers, five internal field borrowers, continuation budget reborrows,
  duplicate status-row source receiver, and scalar predicates. It is paid on
  each actual query through the original full-lookup reservation frontier.

The first activation also pays the entire original `S`, independently of `A`.
Every query retains both complete old compound
`Result<Option<&Option<CallFact>>, Diagnostic>` terms. There is no per-node
comparison credit, old Result credit, canonical-ID credit or old-fee exemption.
One owner adds twelve initialization work units. Every original ordered query
work event and byte comparison remains, including minimum/maximum probes,
certified suffix bounds and tree traversal. All new refusal paths use the
existing sticky prepaid diagnostic mechanism.

For actual query count `N` and owner activations `M`, the conservative byte
opportunity is `(N-M)*S - sum(A_j) - sum(R_i)`. For this one-owner route the
successful threshold is `floor((A+S)/(S-R))+1`, if `S>R`; otherwise retention is
ineligible. This is a source tariff expression, not a measured allocation or
runtime result. Early semantic errors may pay acquisition without amortizing it.
No future-loop/query payment occurs at acquisition.

## Qualification and source custody

The complete immediate-old prefix algorithm and Arena dispatch are independently
compiled in `arena/prefix/retained_before.rs`. The complete immediate-old
certified status receiver and ordinary caller are independently compiled in
`case_certified/lookup_before.rs` and `lookup_caller_before.rs`. The latter uses
the unchanged live old Arena lookup; source assertions bind that lookup to the
complete separately compiled predecessor. Full historical source overlays remain
necessary for independently built whole-composition qualification.

Authored controls cover complete borrowed hit/miss results, noncall/absent
identities, distinct allocations, NUL/UTF-8/prefix endpoints, small/large AVL
shapes, all old query work cuts and six diagnostic fields, exact/one-short
first and later payments, sticky retry, source lifetime custody, nested/foreign
existing-owner refusal, successful/refused/unwind Drop, and whole old/current
ordinary composition at the unchanged cap. These controls have not run in this
source-only lane. General source binders and inventory metadata are owned by the
combined integration batch.

Before promotion, run that combined focused batch with one build job: exact old
and current full callers, mandatory original-ledger crosschecks, codec/catalog/
starter owners, source inventories and scoped production Clippy. Instrument
actual eligible `N`, activations `M`, every `A`/`R` payment, byte/work refusals,
reserved floors, allocator allocated/reserved/reallocated/peak/live metrics and
Drop. Require a positive measured whole-composition difference and disclose
cold, tiny, early-error, dense/miss and fallback costs. Edge-spanning retention
and the remaining broad #885 acceptance controls stay open.
