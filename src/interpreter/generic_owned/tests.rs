use super::match_result_is_admitted;
use crate::hir::{self, OwnershipMode, ResolvedExprKind, ResolvedMatchPattern, ResolvedType};

const SOURCE: &str = include_str!("../../../tests/fixtures/nested-collection-match-results.spx");

#[test]
fn collection_match_normal_and_terminal_results_require_exact_owned_pattern_and_completion() {
    let ast = crate::check(SOURCE, std::path::Path::new("collection-match.spx")).unwrap();
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    for id in ["convert", "terminal"] {
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == id)
            .unwrap();
        let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
            panic!("block")
        };
        let admitted = |expression: &hir::ResolvedExpr| {
            let ResolvedExprKind::Match {
                scrutinee, arms, ..
            } = &expression.kind
            else {
                panic!("match")
            };
            match_result_is_admitted(&program, function, expression, scrutinee, arms)
        };
        assert!(admitted(tail), "{id}");
        if id == "terminal" {
            assert_eq!(tail.ty, ResolvedType::Never);
            assert!(function.cleanup_plan.lexical_control.is_some());
        }
        for mutation in 0..9 {
            let mut forged = tail.as_ref().clone();
            let ResolvedExprKind::Match {
                mode,
                scrutinee,
                arms,
            } = &mut forged.kind
            else {
                unreachable!()
            };
            let ResolvedMatchPattern::Record {
                record,
                instance,
                fields,
            } = &mut arms[0].pattern
            else {
                unreachable!()
            };
            match mutation {
                0 => *mode = hir::ResolvedMatchMode::Borrow,
                1 => scrutinee.ownership = OwnershipMode::Borrow,
                2 => *record = hir::DeclarationId::new("foreign.report"),
                3 => *instance = ResolvedType::Bool,
                4 => fields[0].field = hir::DeclarationId::new("foreign.words"),
                5 => {
                    let hir::ResolvedRecordMatchFieldPattern::Binding(binding) =
                        &mut fields[0].pattern
                    else {
                        unreachable!()
                    };
                    binding.ty = ResolvedType::String;
                }
                6 => {
                    let hir::ResolvedRecordMatchFieldPattern::Binding(binding) =
                        &mut fields[0].pattern
                    else {
                        unreachable!()
                    };
                    binding.ownership = OwnershipMode::Borrow;
                }
                7 => {
                    let hir::ResolvedRecordMatchFieldPattern::Binding(binding) =
                        &mut fields[0].pattern
                    else {
                        unreachable!()
                    };
                    let ResolvedType::Nominal { arguments, .. } = &mut binding.ty else {
                        unreachable!()
                    };
                    arguments[0] = ResolvedType::Bool;
                }
                8 => {
                    forged.ownership = if id == "terminal" {
                        OwnershipMode::Own
                    } else {
                        OwnershipMode::Value
                    }
                }
                _ => unreachable!(),
            }
            assert!(!admitted(&forged), "{id} mutation {mutation}");
        }
    }
    let mut forged = program.clone();
    let function = forged
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "terminal")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
        unreachable!()
    };
    let ResolvedExprKind::Match { arms, .. } = &mut tail.kind else {
        unreachable!()
    };
    // An ordinary value bearing the completion marker is no exit proof.
    arms[0].value.kind = ResolvedExprKind::Int(0);
    let function = forged
        .functions
        .iter()
        .find(|f| f.id.as_str() == "terminal")
        .unwrap();
    let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
        unreachable!()
    };
    let ResolvedExprKind::Match {
        scrutinee, arms, ..
    } = &tail.kind
    else {
        unreachable!()
    };
    assert!(!match_result_is_admitted(
        &forged, function, tail, scrutinee, arms
    ));
    assert_eq!(hir::validate(&forged).unwrap_err().code, "SPX-H006");
    // Collection-record loop construction stays closed: only the hoisted
    // named owner may cross the loop return boundary.
    let wrapped = "Wrapped{report:Report{words:words},good:true}";
    let constructed = SOURCE
        .replace(&format!("let outcome={wrapped};"), "")
        .replace("return outcome;", &format!("return {wrapped};"));
    assert_ne!(constructed, SOURCE);
    let diagnostics =
        crate::check(&constructed, "collection-match-loop-construct.spx").unwrap_err();
    assert!(diagnostics.iter().any(|d| d.code == "SPX-T252"
        && d.message == "record construction is not yet admitted in while bodies"));
}
