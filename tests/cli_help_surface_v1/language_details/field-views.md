### Project field-view details

V31 language-command-io.collection-record.v1: private typed records/admitted Vec;
entry/command fn() -> i64. `vec_field<Row>(value.items,index,"field")`: live named
Vec path; `index` is `usize`; literal field (SPX-T310).
Copy returns by value; String/Bytes return nonescaping borrow str/Slice<u8>.
End views before moving/replacing/sorting/pushing/reserving/clearing roots;
temporary/call roots and escaped/stale views refuse.
`str_as_bytes(vec_field<Row>(value.items,index,"text"))` fuses reads;
`string_as_str(value.inner.label)` keeps the live record path. Avoid vec_clone_at
for queries: it clones owners. These reads apply only in v30/v31/v32, not public ABIs;
gate pending: [scoped reads](SCOPED-VEC-FIELD-READS-V1.md).
