### Scoped record traversal details

`for borrow row at index in rows { body }` adds a scoped field-only row view
for an immutable named vector of an admitted Copy record or flat owned-leaf
record. Omit `at index` when no index is needed. Scalars copy; `row.text` and
`row.bytes` borrow their stored String/Bytes until the iteration ends. The whole
vector stays frozen through the loop, including Copy-only reads. A flat
owned-leaf row may also be lent whole, only as a direct argument to an ordinary
`borrow` parameter of its exact record whose effect-free helper reads just
Copy scalar fields and returns a Value Copy scalar; every other use of the row stays refused. Qualification
is pending; see `help language author:record-traversal`,
[Scoped Record Vec Traversal v2](SCOPED-RECORD-VEC-TRAVERSAL-V2.md) and
[Scoped Record Row Loan v1](SCOPED-RECORD-ROW-LOAN-V1.md).
