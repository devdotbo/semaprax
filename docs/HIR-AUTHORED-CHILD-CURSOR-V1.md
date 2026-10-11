# HIR authored child cursor v1

Status: implementation authored; owning executable qualification pending. OPT [#750](https://github.com/wavect/semaprax/issues/750) remains open.

Audience: compiler contributors and reviewers.

## Summary

The borrowed `hir::lexical_exit::ChildCursor` enumerates the exact authored
child order of the existing indexed `child(expression, index)` API without
restarting a block's statement prefix or a match's arm prefix for every child.
It adds no source admission, schema, runtime authority or backend feature.

## Order and storage contract

A cursor holds an immutable expression reference and fixed-size continuation
state. Block order is each statement's children, then the block tail. While
statements retain condition then body. Match order is scrutinee, then each
present guard and its arm body. Return operands and every other supported HIR
kind retain the existing indexed order. Exhaustion is permanent. The indexed
API remains available as an independent order oracle and for existing callers.

The cursor has no heap backing and does not clone identities or payloads.
A consumer with allocated frames must reserve the actual new frame layout;
reusing a previous smaller frame reservation would be invalid. A cursor grants
no permission to omit a child, duplicate-ID check, source proof or work debit.

## Bounds and adoption

The lexical-presence `requires` and `contains` scans use the cursor with their
unchanged depth guard and fail-closed excessive-depth behavior. The selected structural call lookup also uses the cursor and reserves its
actual `Option<ChildCursor>` frame layout before allocation. The selected primitive-profile and renewal walks, and the resolved nonallocating
helper proof, retain their actual existing frame reservations and use saved
cursor progress. Other callers keep their own independently bounded traversal.
The existing work limits and selected depth bounds are unchanged; cursor
progress is not permission to raise a limit or discount actual allocation.

## Saved-cursor work model

Selected primitive-profile and renewal walks charge one unit for each frame
step: first entry, each child advance, and the terminal exhausted advance.
A block additionally prepays its complete statement census once before scanning
bindings. That census also bounds the cursor's total statement-boundary advances;
it is never charged again as a full-width prefix for each child. Match advancement
is constant: one scrutinee, then each optional guard and body in authored order.
The primitive-profile walk retains its separate complete pattern-node debits.
Its ReplayBudget remains shared across requires, body and ensures; the renewal
counter remains shared across both reconstruction passes.

Resolved helper proofs keep the frame-step debit and a conservative additional
two units for each saved child advance, including exhaustion. The block entry
scan, current-callee lookup and active-function depth scan keep their independent
debits and checks. The source helper proof still uses an indexed child API and
retains its prefix-rescan charges. This change does not audit every existing
signature callback or type classifier, nor does it alter runtime execution fuel.
The helper bound remains 1,000,000 units, selected primitive replay retains its
32,000,000-unit enclosing budget, and selected renewal retains its existing
1,000,000-unit combined counter. No depth or physical bound increases.

For a scalar block with w one-child let statements and one scalar tail, the
selected primitive/renewal walk uses 4w+5 units. The resolved helper uses 8w+13,
including function body and terminal probes. These totals describe performed
saved-cursor work rather than the obsolete quadratic indexed-prefix estimate.

The owning [lexical exit contract](LEXICAL-STRUCTURED-EXITS-V1.md) and
[cleanup contract](RFC-0003-CLEANUP-AND-RESOURCE-ABI.md) retain authority.
Independent selected replay accounting remains subject to its complete
constructor closure; this cursor alone does not complete OPT #745.

## Authored owning gates

`hir::lexical_exit::walk::tests::` compares exact borrowed child pointers against
the indexed API on checked mixed blocks and exit operands, wide mixed statement
blocks and guarded matches. A forged excessive-depth witness retains refusal
and fail-closed detection. These definitions have not been executed in this
implementation tranche. No measured speed, allocation reduction or completed
product qualification is claimed.

Additional authored gates are
`cleanup_plan::replay::selected_profile::cursor_work_tests::` (three bodies),
`cleanup_plan::renewal_profile::selected::cursor_work_tests::` (one body), and
`loop_calls::byte_writer::resolved::cursor_work_tests::` (two bodies).
They bind wide and deep exact/one-short work, terminal probes, complete pattern
and contract-root charges, shared renewal passes, physical exact/one-short frame
reservation, unchanged depth refusal and late unsupported storage/callee refusal.
They have not been executed in this tranche.
