# OPT810 independent oracle funding audit

Status: source audit and residual recommendation only. OPT810 remains open.
No compiler, Cargo, build, test, runtime allocation probe, or new Claude call ran.
No Rust, observer, cap, debit, assertion, receiver, or source-audit inventory changed.

Review base: `d106547c61ba752a6d716c3ae6bbe3a006268ba9`.
Provided failed-run source: `58ee1bcad7d0bf4b714ddcb76e527962f404ea17`.
The two complete provided logs and complete source bytes for both revisions are
saved beside this note in `source-provenance.json`; their SHA-256 hashes bind the
archive. This archive has no replay or fixture-authentication authority.

## Concrete failure and ownership

Both saved failures panic inside the test-only `cross_check` assertion in
`src/cleanup_plan/replay/factored.rs:1069`. Their actual returned error says the
physical reservation exhausted during the factored canonical state memo node.
That is resource refusal, not established skeleton inequality.

The automatic hook is at `factored.rs:119`, after small-function factored
nonselection. Its independent enumerating ledger and independent factored ledger
use `ReplayBudget::with_skeleton_limit`, whose allocation payer is ActiveBuilder
whenever an active bounded builder exists. They therefore consume the same live
invocation budget despite their separate semantic-work counters. The factored
memo node reservation is real: no subtraction, baseline rebasing, or estimate
explains it away. The complete current enumerating and factored receivers stay
unchanged, as do the complete failure tests and original assertions.

The existing APIs cannot provide detached funding within that live phase:

* `bounded_output::with_limit_usage_refusal` restores the parent and charges all
  child consumption into it. Even child consumption preceding parent refusal is
  charged. A nested limit does not create independent physical funding.
* `PhysicalReservation::reserve` calls `reserve_active_required(bytes)`, so the
  selected reservation is also subordinate to that active builder. Selecting it
  changes admission/setup and does not isolate an ordinary test oracle.
* LegacyUnobserved makes reservations succeed without physical payment. The
  earlier Claude suggestion to use that payer is rejected; it would leave actual
  constructors unpriced while the whole allocation observer is active.
* The actual allocation observer is thread-bound. Moving oracle work to another
  thread would require an explicit separately observed/funded full receiving
  phase plus funding for thread setup and transfer. Merely spawning a thread
  would exclude physical work from the original whole observation. No such
  shortcut or new thread surface is introduced here.

## Why the two-test relocation is insufficient

The hook receives `dyn ReplayContext`. Its authority includes the complete
current declarations; ordered type/function views (possibly spread across held
modules); interfaces; templates; instances; call-target lookup; helper
lease/identity; primitive renewal bindings; selected-flat routing; and checked
allocation projection. Two equal source literals or a matching `helper` ID do
not prove those authorities equivalent. In particular, the factored comparison
has a test-only borrowed-helper canonical receiver selected by a current lease.
An ordinary separately resolved program cannot silently stand in for it.

The automatic hook is reached from replay on every small function, including
fresh, borrowed, reused, and hostile contexts. A blanket active-limit skip would
remove that coverage beyond the two supplied failures. The supplied fixtures
also exercise multiple resolutions, receiving results, cache/body reuse, first
refusal, and same-owner retries. Replacing just their first successful call with
an outside oracle would not bind every later context and would not establish
unchanged whole-observer coverage.

## Required implementation follow-up

1. Add an explicit independently paid and allocation-observed oracle phase
   outside both the live invocation ledger and its whole allocation observer.
   Run the complete original enumeration and the complete factored receiver on
   each exact context/function that would otherwise reach the automatic hook.
   Observe acquisition through receiving Drop, retain separate unnormalized
   physical receipts, and retain the existing semantic-work limits.
2. Bind a replacement coverage witness to every semantic input read through
   ReplayContext, including routing/lease inputs and exact full function bytes.
   Use complete equality or authenticated existing immutable provenance; an ID,
   source-only digest, prior-head result, or implementation-produced verdict is
   insufficient. Witness construction belongs to the independent phase, and
   active witness verification must itself be paid or allocation-free.
3. Before removing automatic oracle execution from an invocation, require an
   executable exact witness match. Uncovered active callers must fail closed or
   retain the complete paid oracle, never silently skip it. Establish the full
   caller/fixture replacement inventory before changing the hook.
4. Keep all original application assertions, complete full receivers, literal
   limits, allocation-through-Drop scopes, exact/one-short/floor cases and sticky
   retries. Add controls for changed function/context/routing/lease, missing
   coverage and resource refusal. Missing oracle coverage and multiplicity
   undecided are not semantic mismatch or successful replay.
5. Separately classify oracle outcomes: semantic inequality, multiplicity
   undecided, enumerating physical refusal, and factored physical refusal.
   Preserve the requirement of `Some(true)` where the old enumerating receiver
   proved equality. An error is never an equality result; refusal wording alone
   is not a reliable authority discriminator.

This requires an owning harness/provenance design spanning all active callers;
it is not a bounded two-fixture edit. The safe result of this lane is an exact
residual recommendation rather than an unpriced payer or weakened test. The two
provided failures remain unresolved and must remain visible in consolidated
qualification. Five exact existing selectors are in `pending-selectors.txt` for
the owning lib harness (`cargo test -p semaprax --lib NAME -- --exact`); they are
pending and are not a complete qualification inventory for a future relocation.

Production observations remain separate: the factored fallback for path counts
below MAX_REPLAY_PATHS can return ordinary nonselection after a physical error,
and the selection precheck uses a fresh work budget. Neither behavior was changed
or asserted to be the cause of the supplied test-only panic. Any production
refusal fix needs its own original/current protocol controls.
