# Lexical structured exits v1

Audience: coding agents, compiler contributors and reviewers.

Status: implementation in progress. No source, HIR, cache, cleanup, interpreter,
native or Wasm qualification is asserted by this design. OPT #734 remains Partial.

This tranche admits unlabeled `break;`, `continue;` and `return value;` over the
already admitted carrier families. It adds no residual protocol, comparator,
ambient capability, label, closure escape or new generic/aggregate family.
`?` retains its existing independently specified restrictions.

## Source and typed flow

An exit is a terminal block tail with no normal value. It is not an integer
zero, searching Boolean, discarded value or source-only/backend-only jump.
`LexicalExitKind` is Break, Continue or Return. The authored AST and HIR node is
`LexicalExit { kind, value }`, with the HIR also retaining `target: ControlTarget`.
Break/continue have no value; return has exactly one evaluated value. Source
syntax cannot mint a target. Parsing rejects statements after an unconditional
exit instead of deleting their effects. A conditional with one exit alternative
joins its other normal type; two exit alternatives have no normal completion.
No bottom value may become a parameter, field, ABI slot, local or published result.
An owned record-match arm whose entire result flow exits abruptly retains its
arm region in the abrupt-only certificate. It creates neither a second normal
scope exit nor a normal match-result transfer; independent replay still rejects
a missing/repeated region or a forged normal endpoint.
The initial profile admits exits only on direct function/loop/control-block
statement-tail routes. An exit cannot cross initializer, call argument, aggregate,
return operand, yield, unsafe or closure evaluation. SPX-T312 owns that refusal;
SPX-T313 rejects an absent enclosing loop. Ordinary failures inside a returned
expression or staged call argument remain admitted with existing atomic cleanup.
After ordinary evaluation completes, a statement-shaped exit also settles retained
ordinary owned String literal/read clones in the contour it leaves. Each temporary
must resolve to the exact current owned String clone expression and inventory slot;
a temporary-shaped identity alone is insufficient. Live staged call arguments,
provisional results and other unfinished temporary kinds remain closed. Root locals
remain live through ensures; this does not change String read cloning or the allocator
failure contract. The reference interpreter retains the actual completed operands
in exact temporary/flag slots, including comparison operands and normal scoped
String results. Skipped lazy operands remain inactive; canonical condition,
region and exit finalizers release only their authenticated carriers. Frame and
slot backing are reserved before allocation when an active allocation budget is
held. Owning executable qualification of this repair remains pending.

`ControlTarget::Function(FunctionExecutionId)` names the current ordinary
function execution, including the exact generic instance. `ControlTarget::Loop
{ owner, site }` names the nearest lexical source loop, where site is the exact
condition ExpressionId of its authenticated resolved While. Closures establish a
fresh function boundary and clear the loop target stack. Outside-loop exits and
cross-function/closure targets fail closed. Source and HIR independently reconstruct
the context, complete target identity, reachability, type and ownership joins.

Resolve source loop targeting before For/ForBorrow lowering. Their generated
While condition and source structural mapping must be retained/authenticated.
While continue targets the next condition. Source For continue targets its
iteration latch: iteration cleanup first, then the exact generated advance once,
then condition/next-item evaluation. Dropping the latch would silently turn a
continue into an infinite loop or repeat an item. The exact retained owning iterator
condition independently authenticates Done as false and Yield as true. Only the
matching Yield replacement arm is runtime-feasible after entering that body.
Full structural HIR validation and inventory/census retain both original arms;
independent replay scans both and narrows only completion/path joins. Constructor
and native/Wasm emission derive the same scoped relation from the actual
condition, body and latch, so an all-abrupt Yield body has no ordinary latch.
The entry ownership union and exact latch restoration checks are unchanged.
Unknown conditions, forged Done leaves, field modes and latch identities grant
no feasibility selection. Owning executable qualification remains pending. SourceForStep carries its exact
step ExpressionId; it is not identified by generated variable spelling.

## Authoring and diagnostics

A guard may end its selected arm with `if skip { continue; };` or
`if done { break; };`. Other paths continue normally. Do not add a dummy value
following an unconditional exit. Keep the loop's ordinary tail on paths that
complete normally. No labeled exit or valued break is admitted.

`SPX-T312` rejects a lexical exit crossing an initializer, argument, aggregate,
return operand, closure, unsafe or yields boundary. `SPX-T313` requires an
actual enclosing loop for break/continue. `SPX-T103` and `SPX-O104` preserve the
function's declared result and owning transfer. Independent HIR validation uses
`SPX-H006` for forged function/nearest-loop targets, latch identities or stored
bottom types. A returned call may fail through the ordinary staged-argument
failure route; that does not authorize another lexical exit in its argument.

A loop return may hand off one direct, whole named owned binding into the
function result domain: that owner need not be restored to the loop entry.
This exemption grants no call, effect, allocation or invocation authority.
Every other return operand retains the loop's existing executable admission,
and every operand retains ordinary expression/type/ownership and lexical-target
validation. Source and independent HIR must both reject a closed loop operation
hidden inside `return`, even though the returned expression executes at most once.

## Finite cleanup proof

The selective proof is `semaprax.lexical-control-flow.v1`, joined with selective
cleanup v18 and loan-plan v3. Functions
without lexical exits keep their prior graph/cache/cleanup schemas and bytes.
Older proof profiles refuse new exit nodes rather than guessing an interpretation.

Existing cleanup while proof linearizes one iteration. Keep that finite model,
with an explicit typed loop certificate binding target, condition/body/step IDs,
loop region, after-loop block, and canonical entry liveness/history. A LoopLatch
continuation is a checked handoff to that certificate, not an arbitrary CFG
back-edge. Replay verifies exact entry state restored, every exited region's
canonical finalizers, all source/HIR identities, no staged call epoch or renewal
reservation left outstanding, and precise control kind. It does not enumerate
unbounded iterations or reconstruct cleanup from generated C/Wasm blocks.

Break settles iteration locals and traversed inner regions, retains outer owners,
and joins the same after-loop state as the zero-iteration condition-false edge.
Continue settles the same local regions before the latch/condition. An outer owner
cannot disappear on an exit path unless the independently admitted loop invariant
and after-state permit it. ForBorrow aliases and TraversalFreeze obligations end
at the exact contour. ForOwn continues preserve the remaining iterator and settle
an unmoved current item; break/return settle the remaining owned iterator in its
canonical region order. Existing ownership commit and renewal rules remain exact.

Return first evaluates its expression left to right. Partial construction or
argument failure uses the ordinary selected failure and live-prefix cleanup.
Successful owned return moves into callee ProvisionalResult. It then leaves inner
loop/block regions, retains root locals through ensures, runs every non-result
cleanup action, and reaches the single successful CommitResult epilogue. Each
return has its own checked execution domain; repeated source postconditions reuse
their authenticated source storage/region identities with exact per-domain scope
selection, rather than cloning an unrelated contract or exposing another return's
live owners. The ordinary postcondition temporary cleanup remains mandatory. Failure
in the return expression or ensures never publishes; cleanup cannot replace the
selected status or reacquire arguments committed to a callee. Borrowed publication
uses the existing independent lifetime law, with no new borrowing privilege.

### Backend execution guards

Selective v18 also records proof-only function-entry and successful loop-condition
boundaries. Each binds the exact expression, canonical block and region, exited
region chain, and complete ordered finalizer vector from the authenticated live
builder state. Independent replay reconstructs every boundary, including repeated
resume domains. Native call-depth refusal and private Agent-stage semantic-fuel
exhaustion retain their existing status domains, stickiness and charge points;
they settle only the selected boundary before the common ABI failure epilogue.
Parameter carriers and flags are initialized before entry settlement. A boundary
is neither a new ordinary StatusSource nor owner-admission or ABI authority. It
does not make fatal String allocator failure recoverable or establish an OOM
recovery or production measurement claim.

## Consumers and trust boundaries

Interpreter flow carries exact Break/Continue/Return targets separately from
failure. Native emits only validated plan routes and canonical guarded finalizers.
Core Wasm branches use validated lexical-depth bindings and staged result locals,
then the same postcondition/cleanup epilogue. Neither emitter manufactures target
or cleanup authority from source spans, C labels or Wasm stack layout. Cache and
graph replay reject forged/stale targets, missing or extra exits, wrong latch/step,
wrong region contours, reordered finalizers and mismatched result publication.
Source, retained graph and builder census charge every target/certificate/exit,
child, identity clone and path state at existing capacity limits.

The existing acyclic guard-return normalizer remains the compatibility path for
its admitted source shapes and exact canonical value projections. A source body
requiring real lexical exits uses this explicit profile; it must not partially
normalize away one branch while hiding a loop exit. Canonical formatting retains
explicit exit spelling for this profile, deterministically replaying the same
source/HIR control identities. The profile selection itself is independently
reconstructed from source, not a cached assertion.

## Owning regressions and adoption

Define success and stable hostile controls in existing language, owned_data,
cleanup_backends/native, workspace/cache and HIR harness children: nearest nested
loop targets; continue latch exactly once; locals constructed then exited; outer
owners retained; partial aggregate return failure; own arguments before/after
atomic commit; ensures failure; forbidden borrowed publication; unreachable effects;
forged target/region/certificate/schema; missing/extra/reordered finalizers; and
outstanding renewal reservations. Every claimed backend must agree at native O0/O2,
interpreter and Core Wasm with original immutable expected outputs/status/traces.

The authored `examples/shiftsim-typed-record-successor/src/schedule.spx` now uses
nearest-loop break for the first waiting patient, first free server and exhausted
dispatch pair. It preserves ascending scans, priority/arrival/identifier ordering,
event advancement and the peak-queue observation before dispatch. Invalid requests
return the unchanged invalid ScheduleResult before report construction; successful
requests return the original report shape. The command source uses returning error
guards before encoding and retains exactly one stdout write only after successful
validation and encoding. Its acyclic guards may use the existing compatibility
normalizer; the scheduler's actual loop breaks select the lexical profile.

This is unqualified application authorship: the original ShiftSim SPEC, all15
acceptance cases, hostile wire inputs, >65536-byte whitespace and independent exact
output oracle remain unchanged. The owning typed ShiftSim fixture reads these real
source files; canonical formatting, complete Project admission and same-source
backend/application checks must execute after the batch joins. Bounded help and
later matched application measurement remain separate obligations. No measured
advantage or issue closure follows this adoption.

Owning contracts: RFC-0003-CLEANUP-AND-RESOURCE-ABI.md (return staging and publication),
WHILE-LOOPS-V1.md, GENERIC-OWNED-RESULT-V1.md, ACYCLIC-GUARD-RETURNS-V1.md,
SCOPED-RECORD-VEC-TRAVERSAL-V2.md and the exact OPT #734 acceptance.

The graph projects the `Never` completion marker on exit expressions but omits it
from the value type-facts registry: completion has no storage or ABI facts.
An ordinary single-arm record match may also complete only through a terminal
arm. Its expression and arm retain `Never` with Value mode; the independent
lexical contour validator proves the actual exit rather than trusting that type
marker. Forged ordinary values marked `Never` and ownership-bearing completion
remain rejected. The owning two-return cleanup and ShiftSim gates qualify this
repair in the consolidated batch; authored changes alone prove no backend result.

## Fresh contract-local owners (OPT-743)

This authored extension permits a clause-local `Bytes` created by literal-sized
`bytes_zeroed` (at most 131072 bytes), a String literal, and available aliases of
those exact owners. A consuming ordinary pure call may receive a named fresh
local after the usual type, loan and staged-argument checks. Constructor results
used directly as owned call arguments remain closed. General owned call results,
owned aggregates, captures, projections, effectful calls and ownership transfer
from function parameters, body locals or the provisional `result` remain closed.
The same ordinary owner cannot be transferred twice. Source and HIR reconstruct
freshness independently for each clause; no graph, cache or supplied Boolean
establishes it. Freshness cannot cross a requires/ensures boundary.

```spx
@id("contracts.check")
fn check(value: own Bytes) -> bool { byte_len(bytes_as_slice(value)) == 2usize }
@id("contracts.main")
fn main() -> i64
    requires { let value = bytes_zeroed(2usize); let moved = value; check(moved) }
    ensures { let text = "ok"; string_len(text) == 2 }
{ 29 }
```

Each contract-local owner retains ordinary canonical cleanup and call commit.
Requires temporaries settle before the body. Every selected return checks its
ensures with its own exact failure/result domain, settles ensures temporaries and
non-result owners, and then publishes. Contract failure remains sticky and must
leave caller result storage uninitialized. Existing root owners and provisional
results remain protected until their ordinary settlement/publication point.

The allocation census is cumulative across requires, body and ensures: 32 byte
allocation events and 2097152 statically charged bytes, without refund when a
local is released. Clause freshness does not grant runtime allocation recovery;
ordinary native String/Bytes physical OOM retains its existing contract. OPT-724
remains a separate obligation. Regression definitions cover source/HIR origin
forgery, aliases, duplicate transfer, clause isolation, exact/+1 resource bounds,
canonical/graph/cache replay, native O0/O2 cleanup and Core-Wasm/interpreter
refusal/reentry. These definitions are authored, not evidence of a passing gate.

## Retained primitive replay backing (OPT-745, OPT-749)

The selected retained-replay constructors are authored; executable qualification
is pending. They preserve ordinary cleanup order, source/HIR authentication and
work limits. The first selected context is module-local and primitive; imports,
nominal carriers, generic declarations and delegated runtime providers remain
closed. This restriction is a replay boundary, not broader application support.

Structural exit validation compares borrowed parent links and exact leave order
without constructing an expected-region vector or ancestor tree. CFG traversal
reads authenticated successors directly and reserves its colors and stack before
construction. Selected vector and FIFO frontier growth request `max(required, 2 * capacity)`
through checked arithmetic, charging the complete replacement backing while the
old backing remains charged. Spare capacity cannot refund or reset a refusal.
Ordinary replay retains its existing allocation route.

Owning regressions observe requested allocator Layout bytes and overlapping
backing independently of reservation arithmetic. They retain exact/+1 source
limits and one-short/sticky backing refusal; these observations do not establish
RSS bounds or measured agent-token, billed-cost or application speed gains.

## Iterator composition qualification

The constructor declares lexical status producers in complete typed authored
postorder before CFG lowering. Continue copies reuse exact producer identities;
an unreachable ordinary latch does not remove or reorder its authored status.
The declaration walk and CFG membership checks share one bounded work counter,
and status backing is reserved before construction.

Graph composition authenticates a lexical overlay from retained typed HIR, its
exact execution owner and independent cleanup replay. A schema tag alone, or a
frozen projection without its retained program, supplies no overlay authority.
Wasm repeats the authored owning-iterator Next only inside distinct certified
resume contexts: its exact argument carrier is saved and restored around the
complete latch. Duplicate carriers within one context remain refused.

The status exact/one-short, hostile metadata and closed normal-latch regression
definitions require owning executable qualification; this change does not claim
that those gates have executed.
