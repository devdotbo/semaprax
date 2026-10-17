# OPT878 borrowed merge keys: source candidate

This is the key-storage part of issue #878, based on
`36c6deb98720e243b0daaf81e813fbe3ec3b0a2d`. Execution and application
qualification are pending. It does not close #878 or claim that the reported
width-one or arena application reservations fit.

`merge_keys.rs` retains the two complete grouping algorithms and the borrowed
suffix acquisition route. Instead of cloning owned-source, continuation and
commit payloads into a BTreeMap, a sorted vector stores group indices. Lookup
compares the same fields in the same order by borrowed content. The groups
vector remains in first-appearance order. No reference is stored across group
or directory growth, and indices are discarded before publication consumes the
groups. Directory insertion shifts indices; comparison is logarithmic and
index insertion is linear. This changes CPU work, not the materialization-unit
meaning in RFC 0003, and no CPU improvement is asserted.

The original merge work charge precedes all new scratch. A directory header,
borrowed lookup controls and each actual index backing growth are paid before
construction through the existing replay payer. Old member/group/publication
and union requests remain. The old key clone and BTree node requests disappear
because those payloads and nodes are no longer constructed. Existing caps,
sticky refusal, diagnostic floors and authority selection are unchanged.

The complete input `skeleton_work.before.rs` is retained; `before-inputs.json`
binds its whole length and SHA to the base commit. The three old methods are
independently compiled in `merge_keys_before.rs`. The only changes in that
compiled slice are method identifiers (including the old suffix self-call) and
explicit replay visibility required by the new child module. Its statements,
payment requests and literals are preserved verbatim. The production suffix
route still clones the complete borrowed input before merging; no suffix Drop
order change is hidden in the key change.

The four new lib owners under
`cleanup_plan::replay::skeleton_work::merge_keys_tests` compare full ordered
small languages and multiplicities with the complete prior methods; cover
ordinary/selected, lexical and pending-commit cases; compare every original
work-refusal Diagnostic field; and probe cumulative allocation through success,
exact capacity, every smaller physical allowance and destruction. These tests
have been authored and rustfmt parsed; they have not been executed. The metric
owner prints old/current cumulative reservations, allocations and highwater
without assuming that fewer allocations implies a reservation improvement.

Integration must join `merge_keys.rs`, `merge_keys_before.rs` and
`merge_keys_tests.rs` into the three existing raw source inventories and the
MAIN043 definition domain, retaining every prior source owner. The historical
whole-source bindings need an explicit new finite successor generation for
`skeleton_work.rs` and changed inventories. No source-custody refresh, Cargo,
common gate or public qualification was run in this private lane.

Owned suffix consumption remains a separate required part of #878. A caller
frame containing prefixes before suffixes alone does not suffice: moving
original suffix observations into the existing fallible union passes those
owners to a nested frame which destroys them on refusal before the caller's
prefixes. Activation needs an explicit retained union ownership design and
prefix-before-original-suffix refusal proof; simply passing the owned vector to
`merged_paths` is not the proposed implementation.
