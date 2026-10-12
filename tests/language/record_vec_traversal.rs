use semaprax::{format, graph, hir, parse, verify};

const DECL: &str = r#"module app.record_traversal;
@id("row") record Row { @id("row.text") text:string, @id("row.data") data:Bytes, @id("row.number") number:i64, }
@id("app.consume") fn consume(rows:own Vec<Row>)->i64{0}
"#;
fn source(body: &str) -> String {
    format!(
        "{DECL}@id(\"app.main\") fn main()->i64 {{ let initial=vec_with_capacity<Row>(1usize); let rows=vec_push<Row>(initial,Row{{text:\"x\",data:bytes_zeroed(1usize),number:7}}); {body} }}"
    )
}

#[test]
fn borrowed_record_syntax_roundtrips_and_graph_retains_exact_freeze() {
    let source = source(
        "let mut observed=0; for borrow row at index in rows { let text=row.text; let bytes=row.data; observed=observed+row.number+i64_from_usize(index); 0 } consume(rows)",
    );
    let program = parse(&source, "record-traversal.spx").unwrap();
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let canonical = format::canonical(&program);
    assert!(canonical.contains("for borrow row at index in rows"));
    assert_eq!(
        format::canonical(&parse(&canonical, "record-traversal.spx").unwrap()),
        canonical
    );
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let function = resolved
        .functions
        .iter()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    assert_eq!(function.loan_plan.schema, "semaprax.loan-plan.v2");
    assert!(function
        .loan_plan
        .loans
        .iter()
        .any(|loan| loan.cause == semaprax::loan_plan::LoanCause::TraversalFreeze));
    let json = graph::to_json(&program).unwrap();
    assert!(json.contains("\"kind\":\"traversal_freeze\""));
    assert!(!json.contains("core.vec.clone_at"));
    graph::verify_json(&program, &json).unwrap();
}

#[test]
fn row_escape_carrier_transfer_shadow_and_index_assignment_are_refused() {
    for (body, code) in [
        ("for borrow row in rows { let leaked=row; 0 } 0", "SPX-T284"),
        ("for borrow row in rows { consume(rows) } 0", "SPX-T265"),
        (
            "for borrow row in rows { let rows=vec_with_capacity<Row>(0usize); 0 } 0",
            "SPX-T284",
        ),
        (
            "for borrow row at index in rows { index=1usize; 0 } 0",
            "SPX-U101",
        ),
        ("for borrow row at rows in rows { 0 } 0", "SPX-T209"),
        (
            "for borrow row in rows { for borrow nested in rows {0} 0 } 0",
            "SPX-T284",
        ),
    ] {
        let program = parse(&source(body), "record-traversal-hostile.spx").unwrap();
        let diagnostics = verify::verify(&program);
        assert!(!diagnostics.is_empty(), "admitted hostile body: {body}");
        assert!(
            diagnostics.iter().any(|diagnostic| diagnostic.code == code),
            "{body}: {diagnostics:#?}"
        );
    }
}

// Row Loan v1. The admitted shape is exactly a direct ordinary-call argument
// to a `borrow Row` scalar-field reader; it lowers to a versioned descriptor,
// never to an aggregate copy, and selects Graph v82.
const LOAN_HELPERS: &str = r#"@id("app.weight") fn weight(row:borrow Row, bias:i64)->i64
 requires row.number>0
{ row.number+bias }
"#;
fn loan_source(helpers: &str, body: &str) -> String {
    source(body).replacen(
        "@id(\"app.main\")",
        &format!("{helpers}@id(\"app.main\")"),
        1,
    )
}

#[test]
fn row_loan_to_scalar_reader_roundtrips_and_projects_its_descriptor() {
    let source = loan_source(
        LOAN_HELPERS,
        "let mut total=0; for borrow row at index in rows { total=total+weight(row,i64_from_usize(index)); 0 } total",
    );
    let program = parse(&source, "record-row-loan.spx").unwrap();
    let diagnostics = verify::verify(&program);
    assert!(diagnostics.is_empty(), "{diagnostics:#?}");
    let canonical = format::canonical(&program);
    assert!(canonical.contains("weight(row"));
    assert_eq!(
        format::canonical(&parse(&canonical, "record-row-loan.spx").unwrap()),
        canonical
    );
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let json = graph::to_json(&program).unwrap();
    assert!(json.starts_with("{\"schema\":\"semaprax.graph.v82\""));
    let compact =
        serde_json::to_string(&serde_json::from_str::<serde_json::Value>(&json).unwrap()).unwrap();
    assert!(compact.contains("\"kind\":\"row_loan\""));
    assert!(compact.contains("\"version\":\"semaprax.row-loan.v1\""));
    assert!(compact.contains("\"callee\":\"app.weight\""));
    assert!(!compact.contains("core.vec.clone_at"));
    graph::verify_json(&program, &json).unwrap();
    let forged = json.replacen("\"callee\":\"app.weight\"", "\"callee\":\"app.main\"", 1);
    assert_ne!(forged, json);
    assert!(graph::verify_json(&program, &forged).is_err());
    // A program without a row loan keeps its older graph schema.
    let plain = source.replace("weight(row,i64_from_usize(index))", "row.number");
    let plain = parse(&plain, "record-row-loan-plain.spx").unwrap();
    assert!(!graph::to_json(&plain)
        .unwrap()
        .starts_with("{\"schema\":\"semaprax.graph.v82\""));
}

#[test]
fn row_loan_admission_boundary_keeps_every_escape_refused() {
    const ESCAPE: &str =
        "record traversal row cannot escape or materialize aggregate storage; select a field";
    const HELPER: &str = "record traversal row can only be lent to a helper that reads Copy scalar fields of its borrowed parameter";
    for (helpers, body, message) in [
        // Owning transfer, local storage and builtin consumers stay refused.
        (
            "@id(\"app.weight\") fn weight(row:own Row)->i64 {row.number}\n",
            "for borrow row in rows { let used=weight(row); 0 } 0",
            ESCAPE,
        ),
        ("", "for borrow row in rows { let kept=row; 0 } 0", ESCAPE),
        (
            "",
            "for borrow row in rows { let more=vec_push<Row>(vec_with_capacity<Row>(1usize),row); 0 } 0",
            ESCAPE,
        ),
        // Value-mode and generic targets are not borrowed scalar readers.
        (
            "@id(\"app.weight\") fn weight(row:Row)->i64 {0}\n",
            "for borrow row in rows { let used=weight(row); 0 } 0",
            ESCAPE,
        ),
        // V1 cannot lend a row to an aggregate/view result or effectful helper,
        // even when the row itself is used only through a Copy scalar field.
        (
            "@id(\"app.weight\") fn weight(row:borrow Row)->string {\"owned\"}\n",
            "for borrow row in rows { let used=weight(row); 0 } 0",
            HELPER,
        ),
        (
            "@id(\"app.weight\") fn weight(row:borrow Row)->Bytes {bytes_zeroed(0usize)}\n",
            "for borrow row in rows { let used=weight(row); 0 } 0",
            HELPER,
        ),
        (
            "@id(\"app.weight\") fn weight(row:borrow Row)->i64 uses { process.stdout.write } {row.number}\n",
            "for borrow row in rows { let used=weight(row); 0 } 0",
            HELPER,
        ),
        // Helper bodies that store, return-view, forward or capture the row.
        (
            "@id(\"app.keep\") fn keep(row:borrow Row)->i64 {row.number}\n@id(\"app.weight\") fn weight(row:borrow Row)->i64 {keep(row)}\n",
            "for borrow row in rows { let used=weight(row); 0 } 0",
            HELPER,
        ),
        (
            "@id(\"app.weight\") fn weight(row:borrow Row)->i64 {str_len_bytes(string_as_str(row.text))}\n",
            "for borrow row in rows { let used=weight(row); 0 } 0",
            HELPER,
        ),
        (
            "@id(\"app.weight\") fn weight(row:borrow Row)->i64 {weight(row)}\n",
            "for borrow row in rows { let used=weight(row); 0 } 0",
            HELPER,
        ),
        (
            "@id(\"app.weight\") fn weight(row:borrow Row)->i64 {let kept=row; 0}\n",
            "for borrow row in rows { let used=weight(row); 0 } 0",
            HELPER,
        ),
    ] {
        let source = loan_source(helpers, body);
        let source = if helpers.contains("uses { process.stdout.write }") {
            source.replace("module app.record_traversal;", "module app.record_traversal; permit { process.stdout.write }")
                .replace("fn main()->i64", "fn main()->i64 uses { process.stdout.write }")
        } else { source };
        let program = parse(&source, "record-row-loan-hostile.spx").unwrap();
        let diagnostics = verify::verify(&program);
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == "SPX-T284" && d.message == message),
            "{helpers}{body}: {diagnostics:#?}"
        );
    }
}

#[test]
fn copy_record_rows_keep_their_by_value_contract_and_stay_refused() {
    let source = r#"module app.copy_loan;
@id("cell") record Cell { @id("cell.value") value:i64, }
@id("app.weight") fn weight(cell:borrow Cell)->i64 {cell.value}
@id("app.main") fn main()->i64 {
 let cells=vec_push<Cell>(vec_with_capacity<Cell>(1usize),Cell{value:7});
 let mut total=0;
 for borrow cell in cells { total=total+weight(cell); 0 }
 total
}"#;
    let program = parse(source, "record-row-loan-copy.spx").unwrap();
    assert!(verify::verify(&program)
        .iter()
        .any(|d| d.code == "SPX-T284"));
}

#[test]
fn older_scalar_for_and_primitive_owned_vector_profiles_stay_closed() {
    for body in [
        "for row in rows { row.number } 0",
        "let strings=vec_with_capacity<string>(0usize); for borrow row in strings {0} 0",
    ] {
        let program = parse(&source(body), "record-traversal-old-profile.spx").unwrap();
        assert!(verify::verify(&program)
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T284"));
    }
}
