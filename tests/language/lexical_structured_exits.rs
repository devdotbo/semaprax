//! Owning source controls for the selective lexical-control-flow profile.
use semaprax::{format, hir, parse, verify};
fn source(body: &str) -> String {
    format!("module app.exits; @id(\"app.main\") fn main()->i64 {{ {body} }}")
}
#[test]
fn guard_exits_and_loop_return_canonicalize_without_dummy_values() {
    for body in [
        "let mut index=0; while index<4 { index=index+1; if index==2 { continue; }; if index==3 { break; }; 0 } return index;",
        "let mut index=0; while index<4 { index=index+1; if index==2 { return index; }; 0 } return 8;",
    ] {
        let program=parse(&source(body),"lexical-exits.spx").unwrap();
        let diagnostics=verify::verify(&program);
        assert!(diagnostics.is_empty(),"{diagnostics:#?}");
        let canonical=format::canonical(&program);
        assert_eq!(format::canonical(&parse(&canonical,"lexical-exits.spx").unwrap()),canonical);
        assert!(canonical.contains("return "));
        let resolved=hir::resolve(&program).unwrap();
        hir::validate(&resolved).unwrap();
        assert_eq!(resolved.functions[0].loan_plan.schema,"semaprax.loan-plan.v3");
    }
}
#[test]
fn loop_exit_outside_loop_has_stable_source_diagnostic() {
    for body in ["break;", "continue;"] {
        let program = parse(&source(body), "outside-loop.spx").unwrap();
        assert!(verify::verify(&program)
            .iter()
            .any(|d| d.code == "SPX-T313"));
    }
}
#[test]
fn initializer_exit_crossing_is_refused_before_ownership_lowering() {
    let program = parse(
        &source("let value={ while false { 0 } return 7; }; value"),
        "crossing.spx",
    )
    .unwrap();
    assert!(verify::verify(&program)
        .iter()
        .any(|d| d.code == "SPX-T312"));
}
#[test]
fn lexical_exit_unreachable_suffix_is_not_discarded() {
    assert!(parse(&source("while true { break; 99 } 0"), "suffix.spx").is_err());
}
#[test]
fn old_acyclic_return_projection_remains_identical() {
    let authored = parse(&source("if true { return 7; } return 9;"), "old-return.spx").unwrap();
    let ordinary = parse(&source("if true { 7 } else { 9 }"), "old-return.spx").unwrap();
    assert_eq!(format::canonical(&authored), format::canonical(&ordinary));
}
