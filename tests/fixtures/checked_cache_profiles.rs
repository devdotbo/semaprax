//! Exact original row-loan and mixed byte-traversal cache fixture contents.
//! Shared by the production-API integration owner and the private debit trace.
pub fn sources(byte_traversal: bool) -> [(&'static str, String); 3] {
    let app = include_str!("../../examples/frame-payload-project/src/app.spx").to_owned();
    let mut frame = include_str!("../../examples/frame-payload-project/src/frame.spx").to_owned()
        + r#"
@id("loan.row") record LoanRow { @id("loan.row.n") n: i64, @id("loan.row.text") text: string, }
@id("loan.weight") fn loan_weight(row: borrow LoanRow) -> i64 { row.n + 1 }
"#;
    let mut tests = include_str!("../../examples/frame-payload-project/src/tests.spx")
        .replace("module frame_payload.tests;", "module frame_payload.tests;\nuse type @id(\"loan.row\") from frame_payload.frame as LoanRow;\nuse function @id(\"loan.weight\") from frame_payload.frame as loan_weight;")
        .replace("if valid_ok && mismatch_ok", "if valid_ok && mismatch_ok && loan_scan() == 12")
        + r#"
@id("loan.scan") fn loan_scan() -> i64 {
    let first = vec_push<LoanRow>(vec_with_capacity<LoanRow>(2usize), LoanRow { n: 3, text: "a" });
    let rows = vec_push<LoanRow>(first, LoanRow { n: 7, text: "b" });
    let mut total = 0;
    for borrow row in rows { total = total + loan_weight(row); 0 }
    total
}
"#;
    if byte_traversal {
        frame += r#"
@id("bytes.count") fn byte_count(input: borrow Slice<u8>) -> i64 {
    let view = input;
    let mut total = 0;
    for byte in view { total = total + 1; 0 }
    total
}
"#;
        tests = tests.replace("module frame_payload.tests;", "module frame_payload.tests;\nuse function @id(\"bytes.count\") from frame_payload.frame as byte_count;")
            .replace("loan_scan() == 12", "loan_scan() == 12 && bytes_probe() == 4") + r#"
@id("bytes.probe") fn bytes_probe() -> i64 {
    let owned = bytes_zeroed(4usize);
    let view = bytes_as_slice(owned);
    byte_count(view)
}
"#;
    }
    [
        ("src/app.spx", app),
        ("src/frame.spx", frame),
        ("src/tests.spx", tests),
    ]
}
