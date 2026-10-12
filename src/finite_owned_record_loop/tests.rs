use super::*;
use crate::hir::{
    OwnershipMode as O, ResolvedExprKind as K, ResolvedStatement as S, ResolvedType as T,
};
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
fn checked(text: &str) -> crate::hir::ResolvedProgram {
    let ast = crate::check(text, "finite-record.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let again = crate::check(&canonical, "finite-record.spx").unwrap();
    assert_eq!(canonical, crate::format::canonical(&again));
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    program
}
fn pair(
    program: &crate::hir::ResolvedProgram,
) -> (&crate::hir::ResolvedExpr, &crate::hir::ResolvedExpr) {
    let main = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let K::Block { statements, .. } = &main.body.kind else {
        panic!("main block")
    };
    let S::LexicalWhile {
        condition, body, ..
    } = &statements[1]
    else {
        panic!("lexical loop")
    };
    (condition, body)
}
#[test]
fn source_and_current_hir_rederive_finite_record_bound_and_select_discard_wire() {
    for count in [1, 8, 32] {
        let text = SOURCE.replace("count<3", &format!("count<{count}"));
        let ast = crate::parse(&text, "finite-record.spx").unwrap();
        let main = ast
            .functions
            .iter()
            .find(|f| f.stable_id == "app.main")
            .unwrap();
        let crate::ast::ExprKind::Block { statements, .. } = &main.body.kind else {
            panic!()
        };
        let crate::ast::Statement::While { body, .. } = &statements[1] else {
            panic!()
        };
        assert_eq!(source(main, body).unwrap().iterations, count);
        assert!(
            source(main, &body.as_ref().clone()).is_none(),
            "detached source is not a site"
        );
        let program = checked(&text);
        let (condition, body) = pair(&program);
        assert_eq!(resolved(&program, condition, body), Some(count));
        let detached = (*body).clone();
        assert_eq!(resolved(&program, condition, &detached), None);
        let capacity = crate::hir::analyze_byte_data_capacity(&program).unwrap();
        let summary = capacity.function("app.main").unwrap();
        assert_eq!(summary.bytes_copy_sites, count);
        assert_eq!(summary.owned_byte_payload_bytes, u64::from(count) * 3);
        let graph =
            crate::graph::to_json(&crate::check(&text, "finite-record.spx").unwrap()).unwrap();
        assert!(graph.starts_with("{\"schema\":\"semaprax.graph.v84\""));
        assert!(graph.contains("\"kind\":\"copy_discard\""));
        let wire = crate::cache_codec::encode(&program).unwrap();
        let restored: crate::hir::ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
        assert_eq!(restored, program);
        crate::hir::validate(&restored).unwrap();
    }
}
#[test]
fn return_tail_uses_the_same_finite_bound_without_granting_let_crossing() {
    let text = SOURCE.replace("break;", "return 42;");
    let program = checked(&text);
    let (condition, body) = pair(&program);
    assert_eq!(resolved(&program, condition, body), Some(3));
    let main = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    assert!(main
        .cleanup_plan
        .lexical_control
        .as_ref()
        .unwrap()
        .exits
        .iter()
        .any(|e| e.kind == crate::ast::LexicalExitKind::Return));
}

#[test]
fn unsafe_counter_shapes_and_named_initializer_keep_stable_source_refusals() {
    for text in [
        SOURCE.replace("count=count+1", "count=count+2"),
        SOURCE.replace("let mut count=0", "let mut count=1"),
        SOURCE.replace("count<3", "count<33"),
        SOURCE.replace("count<3", "count<0"),
        SOURCE.replace("count=count+1;", "if count==0 {count=count+1;}else{0}"),
        SOURCE.replace("match own outer", "let answer=match own outer"),
    ] {
        let errors = crate::check(&text, "finite-record-refused.spx").unwrap_err();
        assert!(
            errors
                .iter()
                .any(|e| matches!(e.code, "SPX-T252" | "SPX-T312" | "SPX-T267")),
            "{errors:?}"
        );
    }
    let plain="module app;@id(\"app.main\") fn main()->i64 {let mut count=0;while count<3 {let value=bytes_zeroed(3usize);count=count+1;0}count}";
    assert!(crate::check(plain, "arbitrary-loop.spx")
        .unwrap_err()
        .iter()
        .any(|e| e.code == "SPX-T252"));
}
#[test]
fn forged_discard_ownership_counter_and_status_lose_independent_hir_validation() {
    let original = checked(SOURCE);
    for mutation in 0..10 {
        let mut program = original.clone();
        let main = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let K::Block { statements, .. } = &mut main.body.kind else {
            panic!()
        };
        let S::LexicalWhile { body, .. } = &mut statements[1] else {
            panic!()
        };
        let K::Block {
            statements: locals, ..
        } = &mut body.kind
        else {
            panic!()
        };
        let S::CopyDiscard { value, .. } = &mut locals[1] else {
            panic!("explicit discard")
        };
        match mutation {
            0 => value.ownership = O::Own,
            1 => value.ownership = O::Borrow,
            2 => value.ty = T::Bytes,
            3 => {
                value.ty = T::Never;
                value.kind = K::Int(0);
            }
            4 => {
                let K::Match { arms, .. } = &mut value.kind else {
                    panic!()
                };
                let K::Block { statements, .. } = &mut arms[0].value.kind else {
                    panic!()
                };
                let S::Assign { value, .. } = &mut statements[0] else {
                    panic!()
                };
                let K::Binary { right, .. } = &mut value.kind else {
                    panic!()
                };
                right.kind = K::Int(2);
            }
            5 => {
                let status = main.cleanup_plan.status_sources.first().unwrap().clone();
                main.cleanup_plan.status_sources.push(status);
            }
            6 => {
                assert!(!main.cleanup_plan.status_sources.is_empty());
                main.cleanup_plan.status_sources.remove(0);
            }
            7 => main.cleanup_plan.lexical_control = None,
            8 => {
                let K::Match { arms, .. } = &mut value.kind else {
                    panic!()
                };
                let K::Block { statements, .. } = &mut arms[0].value.kind else {
                    panic!()
                };
                let S::Assign { binding, value, .. } = &mut statements[0] else {
                    panic!()
                };
                let K::Binary { left, .. } = &mut value.kind else {
                    panic!()
                };
                let K::Place(place) = &mut left.kind else {
                    panic!()
                };
                place.root = crate::hir::ValueId::local(
                    &crate::hir::FunctionExecutionId::Monomorphic(main.id.clone()),
                    "body.s0.forged",
                );
                assert_eq!(binding.name, "count");
            }
            9 => {
                let K::Match { arms, .. } = &mut value.kind else {
                    panic!()
                };
                let K::Block { tail, .. } = &mut arms[0].value.kind else {
                    panic!()
                };
                let K::If { then_branch, .. } = &mut tail.kind else {
                    panic!()
                };
                let K::Block { tail, .. } = &mut then_branch.kind else {
                    panic!()
                };
                let K::LexicalExit { target, .. } = &mut tail.kind else {
                    panic!()
                };
                *target = crate::hir::ControlTarget::Function(
                    crate::hir::FunctionExecutionId::Monomorphic(main.id.clone()),
                );
            }
            _ => unreachable!(),
        }
        assert!(
            crate::hir::validate(&program).is_err(),
            "mutation {mutation}"
        );
    }
}

#[test]
fn finite_record_source_and_hir_fold_complete_body_events_at_the_unchanged_cap() {
    let two_sites = SOURCE
        .replace("record Outer {@id(\"outer.report\") report:Report,}", "record Outer {@id(\"outer.report\") report:Report,@id(\"outer.extra\") extra:Bytes,}")
        .replace("Outer{report:Report{bytes:bytes_zeroed(3usize)}}", "Outer{report:Report{bytes:bytes_zeroed(3usize)},extra:bytes_zeroed(1usize)}")
        .replace("Outer{report}=>", "Outer{report,extra:_}=>");
    for count in [1, 8, 16] {
        let text = two_sites.replace("count<3", &format!("count<{count}"));
        let ast = crate::parse(&text, "finite-record-cap.spx").unwrap();
        let main = ast.functions.iter().find(|f| f.stable_id == "app.main").unwrap();
        let crate::ast::ExprKind::Block { statements, .. } = &main.body.kind else { panic!("source root") };
        let crate::ast::Statement::While { body, .. } = &statements[1] else { panic!("source loop") };
        assert_eq!(source(main, body).unwrap().iterations, count);
        let program = checked(&text);
        let (condition, body) = pair(&program);
        assert_eq!(resolved(&program, condition, body), Some(count));
        let capacity = crate::hir::analyze_byte_data_capacity(&program).unwrap();
        let main = capacity.function("app.main").unwrap();
        assert_eq!(main.bytes_copy_sites, count * 2);
        assert_eq!(main.owned_byte_payload_bytes, u64::from(count) * 4);
    }
    // A proved trip count alone cannot admit 34 producer executions.
    let excess = two_sites.replace("count<3", "count<17");
    assert!(crate::check(&excess, "finite-record-excess.spx").unwrap_err().iter().any(|d| d.code == "SPX-T267"));
}

#[test]
fn exact_finite_named_slice_copy_counts_actual_extent_and_keeps_unproved_copy_refusal() {
    let text = SOURCE
        .replace(" let mut count=0;", " let input=[1u8,0u8,255u8];let view=array_as_slice(input);let mut count=0;")
        .replace("bytes_zeroed(3usize)", "bytes_copy(view)");
    let ast = crate::parse(&text, "finite-named-copy.spx").unwrap();
    let main = ast.functions.iter().find(|f| f.stable_id == "app.main").unwrap();
    let crate::ast::ExprKind::Block { statements, .. } = &main.body.kind else { panic!("source root") };
    let body = statements.iter().find_map(|s| match s {
        crate::ast::Statement::While { body, .. } => Some(body),
        _ => None,
    }).unwrap();
    assert_eq!(source(main, body).unwrap().iterations, 3);
    assert!(source(main, &body.as_ref().clone()).is_none());
    let program = checked(&text);
    let main = program.functions.iter().find(|f| f.id.as_str() == "app.main").unwrap();
    let K::Block { statements, .. } = &main.body.kind else { panic!("main") };
    let (condition, body) = statements.iter().find_map(|s| match s {
        S::LexicalWhile { condition, body, .. } => Some((condition, body)),
        _ => None,
    }).unwrap();
    assert_eq!(resolved(&program, condition, body), Some(3));
    let capacity = crate::hir::analyze_byte_data_capacity(&program).unwrap();
    let main = capacity.function("app.main").unwrap();
    assert_eq!((main.bytes_copy_sites, main.owned_byte_payload_bytes), (3, 9));
    for changed in [text.replace("count<3", "count<33"), text.replace("count=count+1", "count=count+2")] {
        assert!(crate::check(&changed, "unproved-copy-loop.spx").unwrap_err().iter().any(|d| d.code == "SPX-T267"));
    }
    let plain = "module app;@id(\"app.main\") fn main()->i64 {let input=[1u8];let view=array_as_slice(input);let mut count=0;while count<3 {let copy=bytes_copy(view);count=count+1;0}count}";
    assert!(crate::check(plain, "ordinary-copy-loop.spx").unwrap_err().iter().any(|d| d.code == "SPX-T267"));
}

#[test]
fn finite_record_payload_exact_and_one_iteration_over_keep_the_original_limit() {
    let exact = SOURCE.replace("count<3", "count<16").replace("bytes_zeroed(3usize)", "bytes_zeroed(131072usize)");
    let program = checked(&exact);
    let (condition, body) = pair(&program);
    assert_eq!(resolved(&program, condition, body), Some(16));
    let capacity = crate::hir::analyze_byte_data_capacity(&program).unwrap();
    let main = capacity.function("app.main").unwrap();
    assert_eq!((main.bytes_copy_sites, main.owned_byte_payload_bytes), (16, 2097152));
    let excess = exact.replace("count<16", "count<17");
    assert!(crate::check(&excess, "finite-record-payload-excess.spx").unwrap_err().iter().any(|d| d.code == "SPX-T267"));
}
