# Lexical structured exits v1

Status: implementation in progress. No source, HIR, cache, cleanup, interpreter,
native or Wasm qualification is asserted by this design. OPT #734 remains Partial.

This tranche admits unlabeled `break;`, `continue;` and `return value;` over the
already admitted carrier families. It adds no residual protocol, comparator,
ambient capability, label, closure escape or new generic/aggregate family.
`?` retains its existing independently specified restrictions.

## Source and typed flow

An exit is a terminal block tail with no normal value. It is not an integer
zero, searching Boolean, discarded value or source-only/backend-only jump.
`LexicalExitKind` is Break, Continue or Return. The proposed AST and HIR node is
`LexicalExit { kind, value }`, with the HIR also retaining `target: ControlTarget`.
Break/continue have no value; return has exactly one evaluated value. Source
syntax cannot mint a target. Parsing rejects statements after an unconditional
exit instead of deleting their effects. A conditional with one exit alternative
joins its other normal type; two exit alternatives have no normal completion.
No bottom value may become a parameter, field, ABI slot, local or published result.

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
continue into an infinite loop or repeat an item. SourceForStep carries its exact
step ExpressionId; it is not identified by generated variable spelling.

## Finite cleanup proof

The selective proof is `semaprax.lexical-control-flow.v1`, joined with the next
explicit cleanup schema reserved by the owning cleanup implementation. Functions
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
cleanup action, and reaches the single successful CommitResult epilogue. Failure
in the return expression or ensures never publishes; cleanup cannot replace the
selected status or reacquire arguments committed to a callee. Borrowed publication
uses the existing independent lifetime law, with no new borrowing privilege.

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

Adopt structured exits in original ShiftSim schedule searching paths and command
result routing only after all compiler consumers join, preserving every original
case and output oracle. Bounded help must explain admitted targets, cleanup and
closed contexts. A grouped focused check follows implementation, with later matched
application measurement. No measured advantage or issue closure follows authorship.

Owning contracts: RFC-0003-CLEANUP-AND-RESOURCE-ABI.md (return staging and publication),
WHILE-LOOPS-V1.md, GENERIC-OWNED-RESULT-V1.md, ACYCLIC-GUARD-RETURNS-V1.md,
SCOPED-RECORD-VEC-TRAVERSAL-V2.md and the exact OPT #734 acceptance.
