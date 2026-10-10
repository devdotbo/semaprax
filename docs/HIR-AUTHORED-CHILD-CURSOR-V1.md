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
actual `Option<ChildCursor>` frame layout before allocation. Other selected
replay callers require coordinated migration of their actual physical frame
reservation and work accounting; until then their indexed API remains intact.
The existing 1,048,576-work and selected depth bounds are unchanged; cursor
progress is not permission to raise a limit or discount actual allocation.

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
