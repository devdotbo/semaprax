# Finite owned-row construction v1

Status: authored selective implementation; owning executable qualification pending.
Audience: compiler contributors, agents and reviewers.

## Summary

This narrow capacity profile gives ordinary allocation authority to at most32
independently proved executions of an existing fresh-row construction loop.
It changes no syntax, ABI, allocation quota, fuel model or runtime representation.
It is not [partition](OWNED-BYTE-PARTITION-V1.md) or
[fusion](OWNED-BYTE-CONSTRUCTION-FUSION-V1.md) authority. Count33 and the original
4096-row witness remain SPX-T267; OPT733, OPT746 and OPT747 remain open.

## Exact source and HIR authority

The source module has one explicit nongeneric pure scalar-result function and
one explicit record with exactly one i64, one owned String and one owned Bytes
field. Parameters, contracts, imports, effects and other declaration families
are excluded. The root block begins with fixed-capacity mutable Vec allocation,
a distinct mutable i64 induction variable initialized to0, and an ordinary
while comparing that exact induction variable against the same positive literal
capacity. The repeated block has exactly three statements: one immutable fresh
record, an exact same-Vec push of that whole record, and the exact unit increment.
The discarded body result is0. There are no lexical exits or nested loops in
construction. Field initialization follows declaration order: the induction
scalar, a decoded owned String literal, and direct literal-sized bytes_zeroed.

Count must be at most the unchanged32-event cap. Checked count-times-width must
fit the existing131072 extent and2097152 cumulative payload bounds. The narrower
combined-extent condition preserves the source candidate's proposed future
single-backing limit; it is not a new larger quota or a claim of one allocation.
Each ordinary constructor execution still allocates one independent owner.
String allocation, fuel charging, left-to-right argument staging and failure
selection remain ordinary.

The source capacity fold selects only the exact pointer-held loop and producer
recovered from current source. Ordinary function-body verification borrows the
original template instead of cloning it: an equal cloned expression cannot
inherit exact source-site authority. Only actual generic substitutions create
owned specialization bodies, and those retain their existing independent guards.
Typed HIR separately recovers the actual Vec and
record declarations, field IDs/order/types, whole binding IDs, ownership modes,
capacity literal, comparison, fresh constructor, Bytes builtin target and
literal extent, exact push/store and unit latch. It does not consume source
candidate counts, attached plans, cache labels or graph facts as authority.
A pointer-only discovery gate rejects noncandidate loops before running a suffix
census, preventing repeated borrowed traversals from rescanning the whole
function. Both capacity folds multiply the complete ordinary repeated body summary using
checked arithmetic. The source and HIR proofs must therefore agree with the
actual checked nodes rather than grant a generic allocating while exception.

The suffix is closed to immutable aliases, owned sort, authenticated borrowed
record traversal and scalar/read-only observations. Clone, foreign calls,
unknown producers, owned row escape, further construction and arbitrary nested
loops refuse. HIR independently replays complete traversal freeze structure.
Both loan replay and this proof share the allocation-free structural prefix.
The selected freeze walk uses exact immediate-parent child pointers instead of
an allowed-ID set, borrowed nominal comparison instead of cloned type vectors,
and the same suffix work counter. Its complete fixed cursor/header backing is
reserved before use, while every flat statement scan debits actual work.
Ordinary scope/type/ownership/loan/cleanup validation still authenticates every
node and every live place. Borrowed proof cursors have fixed depth and work
bounds, with required physical scratch charged before construction. One-short
scratch refuses rather than publishing a finite capacity fact.

## Existing representations and required gates

Parser and formatter syntax, cache tags and runtime loop lowering are unchanged.
Cache and semantic graph consumers reconstruct the profile from actual typed
nodes; no new wire field grants allocation permission. Native C11 and internal
Core-Wasm use their existing independent owner constructors, argument commit,
loop cleanup and borrowed-row carriers. Public provider permissions and ABI
profiles are unchanged. Native physical allocation failure remains ordinary
fatal behavior; this work provides no recoverable OOM claim.

Authored lib definitions are `finite_row_construction::tests::` (six bodies):
checked1/8/32 counts and exact events/payload, canonical and graph replay;
count33/original4096 and unsealed source refusal; hostile typed trip, fields,
producer, store, latch and escape; one-short scratch and mutated consumer calls; exact/one-short complete nested
traversal scratch and zero-budget noncandidate discovery; forged freeze latch
and argument/extent identities.
The existing `finite_byte_scratch::tests::` arbitrary-while, stdin and unknown
constructor refusals remain unchanged.

Authored integration definitions are
`owned_leaf_vec::record_traversal::finite_fresh_rows_` (two bodies). The complete
original ascending-row/Unicode/NUL/owned-Bytes oracle is exercised at1/8/32,
with interpreter, native O0/O2 and internal Core-Wasm, native allocation refusal
from the first borrowed read onward, complete settlement and repeated entry.
The singleton success sentinel differs from its failure value, so failed
Unicode/NUL/count checks cannot pass with the empty arithmetic sum.
The Core-Wasm refusal gate injects first, second and fourth push allocation
failure with exact attempt counts, including terminal fourth-row failure, and malformed borrowed
field words while retaining the host's original status/result/leak assertions.
These definitions have not been executed for this tranche. The original4096
physical gate is unchanged and still requires the separate executor contract.
