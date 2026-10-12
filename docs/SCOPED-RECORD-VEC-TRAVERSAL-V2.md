# Scoped Record Vec Traversal v2

Status: authored bounded contract; executable qualification remains separately gated.
Audience: coding agents, compiler contributors and reviewers.

This is the source implementation contract for OPT #733. Executable
qualification is pending. It extends the existing bounded scalar `for`
contract with a separate borrowed-record spelling; older syntax, admitted
scalar types, encoded statement tags and unaffected loan plans retain their
v1 behavior and bytes. Public capability and command profiles are unchanged.

## Source and ownership

```
for borrow row in rows { body }
for borrow row at index in rows { body }
```

`rows` is one immutable, available named binding with type `Vec<R>` and owning
or shared-borrow mode. `R` is an existing admitted explicit monomorphic flat
Copy record, or an existing flat owned-leaf record. This route adds no record
layout, vector capacity, runtime meter, allocation exemption or refund.
Nested records, generic rows, primitive owned vectors, computed/projected
iterables and nested `for` statements remain refused.

The loop snapshots length once, then visits indices `0..length` in ascending
order. Empty vectors execute no body. The optional index is an immutable
`usize` binding. Carrier, row and index names cannot be shadowed inside the
body. Evaluation within the authored body remains left to right, including
lazy boolean evaluation. Existing admitted bounded-loop body rules apply.

The row is source-only metadata: it cannot be copied, returned, captured or
stored as an aggregate. The one admitted whole-row use is the additive
call-only loan of [Scoped Record Row Loan v1](SCOPED-RECORD-ROW-LOAN-V1.md):
a flat owned-leaf row passed directly to an ordinary `borrow R` parameter whose
helper is independently proven to read only Copy scalar fields. Every other
passing form, including Copy-record rows, remains refused. `row.field` selects an exact persistent
field identity. Copy scalars return by value; String fields return `borrow str`
and Bytes fields return `borrow Slice<u8>`. `str_as_bytes(row.text)` uses the
existing authenticated String field byte-view operation. No owned row or leaf
clone is produced. Each iteration has its ordinary local binding/cleanup
scope, and views cannot escape it. The complete vector owner and generation
are frozen throughout the loop, including loops that read only Copy fields.
Mutation, transfer, rebinding and renewal of that carrier are refused inside
the body. Ordinary owner use resumes after the loop and all view expiries.

Statement-tail `break` and `continue` additionally use the selective
[Lexical structured exits v1](LEXICAL-STRUCTURED-EXITS-V1.md) profile. Iteration
views settle before `continue` advances the authenticated index exactly once;
`break` ends the loop freeze before ordinary root use resumes. This composition
retains pending executable qualification and the existing nested-for refusal.

## Independent proof and representation

The additive source statement keeps its row/index spelling in canonical source
and the AST cache. Lowering emits the existing bounded length/index/while
shape and existing typed Vec reads. No new backend aggregate carrier or private
runtime import is introduced. The interpreter, native O0/O2 and Core Wasm use
those existing checked operations and ordinary cleanup plans.

HIR validation reconstructs the length snapshot, zero initial index, ascending
increment, immutable iteration index, nominal layout and exact whole-root
reads independently. Loan planning repeats this reconstruction using the
shared checked-work counter. Source/HIR-only reconstruction is separately
bounded by the same work limit. Workspace replay binds the authored field,
carrier, index and loop identities to retained HIR, including imported record
reference edges; replacing a same-typed field is not authorized by its layout.

Without lexical exits, plans containing the new `TraversalFreeze` cause use
`semaprax.loan-plan.v2`; the selective lexical composition uses loan-plan v3. Its graph cause is `{"kind":"traversal_freeze"}`.
The origin is the exact whole Vec place, the start is before the complete
lowered loop region, and its CFG-qualified end follows that region. The
existing v1 loan count, point, edge and shared work limits remain unchanged.
Unchanged functions keep `semaprax.loan-plan.v1`; nonlexical borrowed-root empty plans
also retain v1 while ordinary HIR proof still checks their traversal freeze.
Canonical cleanup vectors are never reordered or repaired downstream.

Copy-record Vec length, capacity and element reads retain the existing
compiler-owned borrow parameter contract. Loan discovery and overlap replay
authenticate the exact operation, nominal element, whole carrier and argument
shape even when the generated owner-place expression keeps `own` mode. This
does not exempt consuming Vec calls or authorize a move during the freeze.

## Qualification definitions

Owning definitions live in `tests/language/record_vec_traversal.rs`,
`tests/owned_data/owned_leaf_vec/record_traversal.rs`,
`tests/owned_data/copy_record_vec/traversal.rs`,
`src/loan_plan/traversal/tests.rs`, and
`src/workspace_graph/tests/record_traversal.rs`. They cover round-trip/source
refusals, Copy and owned rows, exact index order, repeated iteration views,
owner reentry, native allocation-free reads, strict private Wasm host refusal,
whole-root freeze replay, exact loan inventory capacity, and source-bound
same-typed field rejection. Definitions are not execution evidence.

Application scheduling qualification, allocation-fault sweeps over this
composed loop route, maximum-capacity traversal, and full hostile cache and
backend generation substitution matrices remain unqualified. No isolated
performance gain or completed issue is claimed before the grouped gates.
