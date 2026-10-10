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
