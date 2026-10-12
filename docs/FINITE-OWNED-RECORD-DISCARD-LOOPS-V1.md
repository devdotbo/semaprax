# Finite owned-record discard loops v1

This source tranche is pending executable qualification. It preserves the
ordinary allocation limits and does not authorize arbitrary allocating while
bodies.

A parameter-free, effect-free monomorphic function returning i64 may use a
direct zero-initialized mutable i64 counter followed immediately by a while
whose condition is that binding less than a positive literal at most 32.
The body contains one fresh immutable monomorphic record and one direct
expression-statement owned record match, followed by scalar zero. The match
has one guard-free record arm. Its first and only statement increments the
exact counter by one; its tail may complete normally or use checked Continue,
Break or Return. Additional assignments, nested loops, closures and counter
shadowing are closed. Nested branch Blocks contain no additional statements. Source and HIR independently derive this exact shape
from current declarations and expression pointers.

The bound is conservative even when an exit terminates earlier. Existing
FiniteOwnedLoop capacity flow multiplies each body's actual allocation demand
by this bound. Legacy32's 32 successful Bytes-producing events and 2 MiB
payload remain unchanged; overflow, excessive demand and allocating conditions
still refuse. Induction is not ownership permission: independent type,
availability, loan, canonical cleanup and backend checks remain mandatory.
Every iteration-local descendant must settle before a latch, exit or failure.

## Explicit no-result statement

An authenticated parser Match expression-statement marker containing a lexical exit
lowers to HIR CopyDiscard { value, span }. It evaluates its value once, left to
right, and declares no binding or result storage. Normal completion must be one
of the existing eight Value Copy scalars. Never completion retains the existing
independent lexical target and certificate checks and has no fallthrough result.
Expression temporaries, call staging and arm-local owned bindings retain their
full canonical cleanup and sticky failure transitions.

A named Let initializer with identical match text remains a value boundary and
cannot cross Break, Continue or Return. StatementIf's existing scalar-zero
guard completion restriction is unchanged. Nonlexical expression statements
retain their existing representation.

## Projections and compatibility

CopyDiscard owns checked-HIR codec statement tag 5 and compiler cache
compatibility semaprax.project-checked-module-hir.v10. Decode grants no authority.
Current source, imported identity and complete independent HIR validation
remain required before reuse.

Programs containing CopyDiscard select semaprax.graph.v84, layered over the
existing checked graph. Its statement has kind copy_discard and a value child.
Expression paths remain .sN.value. Programs without this statement retain their
previous graph versions, including row-loan v82 and borrowed byte traversal
v83. Frozen evidence profiles refuse this newer feature.

The original owned-subtree Break/Continue three-backend fixture is retained.
Owning controls independently derive exact source/HIR sites, capacity and wire,
reject detached sites and malformed ownership/counters/statuses, and preserve
ordinary named-initializer refusal. Authored controls are not passing receipts.
