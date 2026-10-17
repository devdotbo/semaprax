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
ReplayBudget loan. The arena's fact-payload lifetime is independent of the
function/proof/status-directory lifetime. Only their common local session loan
is shared; no `Facts`-inner-outlives-function bound is added to the validator. Its query identity loan must last for that entire session:
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

The selector uses a borrowed payer-tag pattern. It creates no owned
`AllocationPayer`, and no equality call or equality actual/formal references.
The disjoint selector site ledger is:

| Constant | Actual new roles paid before formation |
| --- | --- |
| `PAYER_TAG` | One `&AllocationPayer` borrowed scrutinee, two match-arm `bool` values, and one result receiver. |
| `ROW_LENGTH` | Two `&Vec<StatusSource>` roles for `Vec::len` actual/formal receivers and two `usize` roles for returning/receiving its length. |
| `ROW_BOUND` | One `usize` bound operand, primitive comparison `bool`, and its receiver. |
| `SHORT_CIRCUIT` | The `&&` result and the `if` receiver, both `bool`. |
| `FACTS_ARM` | A borrowed `&Facts` scrutinee receiver, the `&Arena` arm binding, and the variant-selection branch `bool`. |
| `NONEMPTY` | Outer `Arena::len` and inner `Vec<Node>::len` actual/formal receiver pairs, their two returning/receiving `usize` pairs, zero operand, primitive comparison result and `if` receiver. |
| `PAYMENT_AND_RETURN` | Eight complete `Result<(), Diagnostic>` roles: four for extra first zero-byte/work authentication, two for selector reservation, and two for helper return/receiving. |
| `PAYMENT_TRANSPORT` | Six `(&mut ReplayBudget, &ResolvedFunction, usize, &str)` tuples: actual/formal transports for the two extra authentication calls and selector reservation. |
| `CONTINUATION` | Both complete actual/formal six-reference transports for `complete_rows`. |

`SELECT_BYTES` sums exactly those named groups; it contains no unformed
`Option<usize>` donor entries. The private `Node` receiver layout is computed in
the arena module, where that actual type is available. No previous held-prefix
selector payment is reused.

The remaining one-owner acquisition term pays two complete `Session` layouts,
two each of its nested `Control`, `Compared` and `Bytes` initializers, both
three-reference constructor transports, four complete acquisition/work
`Result<(), Diagnostic>` roles, four corresponding payment transport tuples,
and two scalar acquisition receivers. The first-frontier reservation includes
this full term before `new_prepaid` forms any owner. These are distinct from
the selector/helper transports and from `R`; they do not precharge a future row.

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
complete separately compiled predecessor. The relocated ordinary body has
only two exact inverse adaptations: remove its `pub(super)` test visibility and
map `crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20` back to the original
`super::CLEANUP_PLAN_SCHEMA_V20`. The constant is fully qualified because the
copied function's parent namespace is different. Full historical source
overlays remain necessary for independently built whole-composition qualification.

Authored controls cover complete borrowed hit/miss results, noncall/absent
identities, distinct allocations, NUL/UTF-8/prefix endpoints, small/large AVL
shapes, all old query work cuts and six diagnostic fields, exact/one-short
first and later payments, sticky retry, source lifetime custody, nested/foreign
existing-owner refusal, successful/refused/unwind Drop, and whole old/current
ordinary composition at the unchanged cap. A separate authored lifetime control
drops independently sourced fact payloads and their function owner before
reading the returned status directory; selector source controls bind the typed
ledger and exact namespace inverse. These controls have not run in this
source-only lane. The existing HIR, semantic-graph and native source-inventory
functions now include all seven complete new modules beside their owning roots.
The two live modules keep production registrations; five independent controls
retain their exact `cfg(test)` edges. A source control binds every edge and one
literal per module per raw inventory. The native file's two finite inline joins
keep its existing 1500-line budget without removing an original include.

The existing prefix, Main8a, Maina3 and region-entry controls acquire named exact
pre-885 inputs before their unchanged historical authorities. The shared
`before_885_source` test helper reverses only the unique complete arena module
registration, directory reexport and new raw-source join expressions. Complete
restored arena/directory/HIR/graph/native source bytes must equal fixed SHA-256
priors from `8c95f77750aaaba9eaa59da865eb65658900646c`. It rejects drift,
truncation and already-historical input; an unlisted input is passed to its
original authority without granting acceptance. The prefix CI transport keeps
the original caller/path and feeds that acquired prior into the existing
CI/semantic-reflow helper. Original snapshots, fixed digests, edit cardinalities,
old producer bodies and final full-source equality assertions remain intact.

Final 241/98/174/F6 metadata, ordered raw-domain tables, complete production/test
class authority and physical pin qualification remain owned by the combined
integration batch. These source joins do not authorize stale metadata or a new
price/budget claim.

Before promotion, run that combined focused batch with one build job: exact old
and current full callers, mandatory original-ledger crosschecks, codec/catalog/
starter owners, source inventories and scoped production Clippy. Instrument
actual eligible `N`, activations `M`, every `A`/`R` payment, byte/work refusals,
reserved floors, allocator allocated/reserved/reallocated/peak/live metrics and
Drop. Require a positive measured whole-composition difference and disclose
cold, tiny, early-error, dense/miss and fallback costs. Edge-spanning retention
and the remaining broad #885 acceptance controls stay open.
