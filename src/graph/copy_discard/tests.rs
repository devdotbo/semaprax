//! Full original finite-record source, exact current replay and frozen controls.
use super::*;
use crate::hir::{ResolvedExprKind as K, ResolvedStatement as S};
const SOURCE: &str = r#"module app;
@id("report") record Report {@id("report.bytes") bytes:Bytes,}
@id("outer") record Outer {@id("outer.report") report:Report,}
@id("score") fn score(value:own Report)->i64 {match own value {Report{bytes}=>i64_from_usize(byte_len(bytes_as_slice(bytes))),}}
@id("app.main") fn main()->i64 {
 let mut count=0;
 while count<3 {
  let outer=Outer{report:Report{bytes:bytes_zeroed(3usize)}};
  match own outer {Outer{report}=>{
   count=count+1;
   if count==1 {continue;}else{if count==2 {break;}else{score(report)}}
  },};
  0
 }
 if count==2 {42}else{0}
}
"#;
fn resolved(source: &str) -> (crate::ast::Program, ResolvedProgram) {
    let ast = crate::check(source, "graph-v84-composition.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    (ast, program)
}
fn assert_full_replay(program: &ResolvedProgram) {
    crate::hir::validate(program).unwrap();
    crate::loan_plan::validate_program(program).unwrap();
    crate::hir::analyze_byte_data_capacity(program).unwrap();
    crate::cleanup_plan::validate_program(program).unwrap();
}
#[test]
fn full_current_finite_discard_composes_flat_loans_with_nested_storage_in_v84() {
    for count in [1, 8, 32] {
        let source = SOURCE.replace("count<3", &format!("count<{count}"));
        let (ast, program) = resolved(&source);
        let score = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "score")
            .unwrap();
        assert!(!score.loan_plan.loans.is_empty());
        assert!(score
            .loan_plan
            .loans
            .iter()
            .all(|loan| loan.origin.projections.is_empty()));
        assert_full_replay(&program);
        assert!(authenticate_finite_composition(&program).unwrap());
        assert_eq!(graph_schema(&program).unwrap(), SCHEMA);
        assert_eq!(
            super::super::nested_owned::pre_filesystem_graph_schema(&program).unwrap(),
            SCHEMA
        );
        let graph = crate::graph::to_json(&ast).unwrap();
        assert!(graph.starts_with("{\"schema\":\"semaprax.graph.v84\""));
        assert!(graph.contains("\"kind\":\"copy_discard\""));
        crate::graph::verify_json(&ast, &graph).unwrap();
        let canonical = crate::format::canonical(&ast);
        let canonical_ast = crate::check(&canonical, "graph-v84-composition.spx").unwrap();
        assert_eq!(canonical, crate::format::canonical(&canonical_ast));
        let canonical_graph = crate::graph::to_json(&canonical_ast).unwrap();
        crate::graph::verify_json(&canonical_ast, &canonical_graph).unwrap();
        let wire = crate::cache_codec::encode(&program).unwrap();
        let restored: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
        assert_eq!(restored, program);
        assert_full_replay(&restored);
        assert_eq!(graph_schema(&restored).unwrap(), SCHEMA);
    }
}
#[test]
fn current_v84_composition_does_not_lend_frozen_or_detached_parts_authority() {
    let (ast, program) = resolved(SOURCE);
    assert_eq!(legacy_graph_schema(&program).unwrap_err().code, "SPX-G410");
    assert!(crate::graph::to_legacy_json(&ast)
        .unwrap_err()
        .iter()
        .any(|e| e.code == "SPX-G410"));
    assert_eq!(
        crate::graph::to_legacy_hir_json(&program, "source-only")
            .unwrap_err()
            .code,
        "SPX-G410"
    );
    assert_eq!(
        super::super::nested_owned::nested_cleanup_graph_schema(
            None,
            program.functions.iter(),
            false,
            false,
        )
        .unwrap_err()
        .code,
        "SPX-G410"
    );
    let detached = program.functions.clone();
    assert_eq!(
        super::super::nested_owned::nested_cleanup_graph_schema(
            Some(&program),
            detached.iter(),
            false,
            false,
        )
        .unwrap_err()
        .code,
        "SPX-G410"
    );
    let main = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let K::Block { statements, .. } = &main.body.kind else {
        panic!("main block");
    };
    let S::LexicalWhile {
        condition, body, ..
    } = &statements[1]
    else {
        panic!("finite loop");
    };
    assert_eq!(
        crate::finite_owned_record_loop::resolved(&program, condition, body),
        Some(3)
    );
    let detached_body = body.as_ref().clone();
    assert_eq!(
        crate::finite_owned_record_loop::resolved(&program, condition, &detached_body),
        None
    );
}
#[test]
fn native_import_flags_precede_current_finite_composition_authentication() {
    let (_, mut program) = resolved(SOURCE);
    assert_eq!(
        super::super::nested_owned::nested_cleanup_graph_schema(
            Some(&program),
            program.functions.iter(),
            true,
            false,
        )
        .unwrap_err()
        .code,
        "SPX-G410"
    );
    let native = crate::hir::resolve(
        &crate::parse(
            r#"module native;
@id("native.host") interface Host permits {} {
 @id("native.ping") import rust fn ping(value:i64)->i64 effects {} failure infallible;
}
@id("native.main") fn main()->i64 {0}
"#,
            "graph-v84-native-flags.spx",
        )
        .unwrap(),
    )
    .unwrap();
    program.interfaces = native.interfaces;
    assert_eq!(graph_schema(&program).unwrap_err().code, "SPX-G410");
}
#[test]
fn forged_flat_loan_cleanup_and_finite_counter_cannot_select_current_v84_composition() {
    for mutation in 0..3 {
        let (_, mut program) = resolved(SOURCE);
        if mutation == 0 {
            let score = program
                .functions
                .iter_mut()
                .find(|f| f.id.as_str() == "score")
                .unwrap();
            score.loan_plan.loans[0]
                .origin
                .projections
                .push(crate::hir::PlaceProjection::Field(DeclarationId::new(
                    "forged.flat.field",
                )));
        } else {
            let main = program
                .functions
                .iter_mut()
                .find(|f| f.id.as_str() == "app.main")
                .unwrap();
            if mutation == 1 {
                main.cleanup_plan.lexical_control = None;
            } else {
                let K::Block { statements, .. } = &mut main.body.kind else {
                    panic!("main block");
                };
                let S::LexicalWhile { condition, .. } = &mut statements[1] else {
                    panic!("finite loop");
                };
                let K::Binary { right, .. } = &mut condition.kind else {
                    panic!("counter bound");
                };
                right.kind = K::Int(33);
                assert!(!authenticate_finite_composition(&program).unwrap());
            }
        }
        assert_eq!(
            graph_schema(&program).unwrap_err().code,
            if mutation == 2 {
                "SPX-G410"
            } else {
                "SPX-H006"
            },
            "mutation {mutation}"
        );
        assert_eq!(
            crate::hir::validate(&program).unwrap_err().code,
            "SPX-H006",
            "mutation {mutation}"
        );
    }
}

#[test]
fn nonfinite_copy_discard_retains_the_existing_nested_projected_loan_route() {
    let (ast, program) = resolved(
        r#"module app;
@id("leaf") record Leaf {@id("leaf.payload") payload:Bytes,}
@id("outer") record Outer {@id("outer.leaf") leaf:Leaf,}
@id("take") fn take(value:own Outer)->i64 {
 match own value {Outer{leaf:Leaf{payload}}=>{return 0;},};
 0
}
@id("inspect") fn inspect(value:own Outer)->usize {
 let view=bytes_as_slice(value.leaf.payload);
 byte_len(view)
}
@id("app.main") fn main()->i64 {0}
"#,
    );
    assert!(crate::hir::copy_discard_program_uses(&program));
    assert!(!authenticate_finite_composition(&program).unwrap());
    let inspect = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "inspect")
        .unwrap();
    assert!(!inspect.loan_plan.loans.is_empty());
    assert!(inspect
        .loan_plan
        .loans
        .iter()
        .all(|loan| loan.origin.projections.len() == 2));
    assert_full_replay(&program);
    assert_eq!(graph_schema(&program).unwrap(), SCHEMA);
    let graph = crate::graph::to_json(&ast).unwrap();
    assert!(graph.starts_with("{\"schema\":\"semaprax.graph.v84\""));
    crate::graph::verify_json(&ast, &graph).unwrap();
    let mut forged = program.clone();
    forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "inspect")
        .unwrap()
        .loan_plan
        .loans[0]
        .origin
        .projections
        .clear();
    assert_eq!(graph_schema(&forged).unwrap_err().code, "SPX-G410");
}
