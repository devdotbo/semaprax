//! Compare target algebra against the independent enumerating typed skeleton.
use super::*;

fn scalar(body: &str) -> ResolvedProgram {
    let source = format!(
        r#"module census.targets;
@id("census.run") fn run(flag:bool,choose:bool)->i64 ensures result==7 {{
{body}
}}
@id("app.main") fn main()->i64 {{0}}
"#
    );
    crate::hir::resolve(&crate::check(&source, "target-census.spx").unwrap()).unwrap()
}
fn run(program: &ResolvedProgram) -> &ResolvedFunction {
    program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "census.run")
        .unwrap()
}
fn enumerated(program: &ResolvedProgram, function: &ResolvedFunction) -> usize {
    let mut budget =
        ReplayBudget::with_skeleton_limit(skeleton_work_upper(program, function).unwrap());
    hir_skeleton_paths(program, function, &mut budget)
        .unwrap()
        .len()
}
#[test]
fn matching_break_continue_return_and_normal_latch_match_small_exact_enumeration() {
    for (body, expected) in [
        ("while flag {break;} return 7;", 4),
        ("while flag {continue;} return 7;", 3),
        ("while flag {if choose {break;}else{0} 0} return 7;", 5),
        ("while flag {return 7;} return 7;", 4),
    ] {
        let program = scalar(body);
        let function = run(&program);
        let (paths, _) = bounds(&program, function).unwrap();
        assert_eq!(paths, expected, "{body}");
        assert_eq!(paths, enumerated(&program, function), "{body}");
    }
}
fn first_break(expression: &mut ResolvedExpr) -> Option<&mut crate::hir::ControlTarget> {
    match &mut expression.kind {
        ResolvedExprKind::LexicalExit {
            kind: crate::ast::LexicalExitKind::Break,
            target,
            ..
        } => Some(target),
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                if let Some(target) = first_break(match statement {
                    ResolvedStatement::While { body, .. }
                    | ResolvedStatement::LexicalWhile { body, .. } => body,
                    _ => statement.value_mut(),
                }) {
                    return Some(target);
                }
            }
            first_break(tail)
        }
        _ => None,
    }
}
#[test]
fn nested_loop_consumes_only_its_break_and_rejects_outer_or_foreign_targets() {
    let original = scalar("while flag {while choose {break;} continue;} return 7;");
    assert_eq!(bounds(&original, run(&original)).unwrap().0, 4);
    assert_eq!(enumerated(&original, run(&original)), 4);
    let function = run(&original);
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("block")
    };
    let ResolvedStatement::LexicalWhile { condition, .. } = &statements[0] else {
        panic!("outer loop")
    };
    let outer = condition.id.clone();
    for foreign in [false, true] {
        let mut program = original.clone();
        let function = program
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "census.run")
            .unwrap();
        let target = first_break(&mut function.body).unwrap();
        let crate::hir::ControlTarget::Loop { site, owner } = target else {
            panic!("loop target")
        };
        if foreign {
            *owner = crate::hir::FunctionExecutionId::Monomorphic(crate::hir::DeclarationId::new(
                "other.run",
            ));
        } else {
            *site = outer.clone();
        }
        assert_eq!(
            bounds(&program, run(&program)).unwrap_err().code,
            "SPX-H006"
        );
        assert_eq!(crate::hir::validate(&program).unwrap_err().code, "SPX-H006");
    }
}
#[test]
fn generated_copy_and_owned_advancement_paths_are_counted_from_actual_hir() {
    for (ty, header) in [
        ("i64", "for value in values"),
        ("Bytes", "for own value in vec_into_iter<Bytes>(values)"),
    ] {
        let mode = if ty == "Bytes" { "own" } else { "borrow" };
        let source = format!(
            r#"module census.targets;
@id("census.run") fn run(values:{mode} Vec<{ty}>)->i64 ensures result==7 {{
{header} {{continue;}} return 7;
}}
@id("app.main") fn main()->i64 {{0}}
"#
        );
        let program =
            crate::hir::resolve(&crate::check(&source, "advance-census.spx").unwrap()).unwrap();
        let function = run(&program);
        assert_eq!(
            bounds(&program, function).unwrap().0,
            enumerated(&program, function),
            "{ty}"
        );
        if ty == "i64" {
            let mut observer = None;
            crate::hir::function_value::walk(function, |expression| {
                if matches!(&expression.kind, ResolvedExprKind::Call { callee, .. }
                    if callee.as_str() == crate::vec_ops::VecOp::Len.id())
                {
                    observer = Some(expression.clone());
                }
            });
            let observer = observer.expect("generated condition retains its Vec length read");
            let counts = expression_path_counts(function, &observer).unwrap();
            assert_eq!((counts.normal, counts.failed), (1, 0));
            let mut capacity = observer.clone();
            let ResolvedExprKind::Call { callee, .. } = &mut capacity.kind else {
                unreachable!()
            };
            *callee = crate::hir::DeclarationId::new(crate::vec_ops::VecOp::Capacity.id());
            assert_eq!(
                expression_path_counts(function, &capacity).unwrap().failed,
                0
            );
            for mutation in 0..4 {
                let mut ordinary = observer.clone();
                let ResolvedExprKind::Call {
                    callee,
                    args,
                    instance,
                    type_arguments,
                } = &mut ordinary.kind
                else {
                    unreachable!()
                };
                match mutation {
                    0 => args.clear(),
                    1 => *callee = crate::hir::DeclarationId::new("user.read"),
                    2 => {
                        *instance = Some(crate::hir::FunctionInstanceId::derive(
                            callee,
                            type_arguments,
                        ))
                    }
                    3 => type_arguments[0] = ResolvedType::Bool,
                    _ => unreachable!(),
                }
                let counts = expression_path_counts(function, &ordinary).unwrap();
                assert_eq!((counts.normal, counts.failed), (1, 1));
            }
            // Census-only control: retain a child's failure without inventing a
            // second observer failure. This is not an executable admission claim.
            let mut child_failure = observer.clone();
            let ResolvedExprKind::Call { args, .. } = &mut child_failure.kind else {
                unreachable!()
            };
            args[0].kind = ResolvedExprKind::Call {
                callee: crate::hir::DeclarationId::new("user.construct"),
                type_arguments: Vec::new(),
                instance: None,
                args: Vec::new(),
            };
            let counts = expression_path_counts(function, &child_failure).unwrap();
            assert_eq!((counts.normal, counts.failed), (1, 1));
        }
        let mut malformed = program.clone();
        let function = malformed
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "census.run")
            .unwrap();
        fn spoil(expression: &mut ResolvedExpr) -> bool {
            let ResolvedExprKind::Block { statements, .. } = &mut expression.kind else {
                return false;
            };
            for statement in statements {
                if let ResolvedStatement::LexicalWhile { latch, .. } = statement {
                    match latch {
                        crate::hir::LoopLatchKind::SourceForStep { step, .. } => {
                            *step = ExpressionId::from_owned("wrong.advance".to_owned())
                        }
                        crate::hir::LoopLatchKind::OwnedIteratorStep { next, .. } => {
                            *next = ExpressionId::from_owned("wrong.advance".to_owned())
                        }
                        _ => panic!("generated latch"),
                    }
                    return true;
                }
                if spoil(match statement {
                    ResolvedStatement::While { body, .. }
                    | ResolvedStatement::LexicalWhile { body, .. } => body,
                    _ => statement.value_mut(),
                }) {
                    return true;
                }
            }
            false
        }
        assert!(spoil(&mut function.body));
        assert_eq!(
            bounds(&malformed, run(&malformed)).unwrap_err().code,
            "SPX-H006"
        );
    }
}
#[test]
fn exact_skeleton_reservation_covers_walk_and_one_short_refuses_before_materialization() {
    let program = scalar("while flag {if choose {break;}else{continue;} } return 7;");
    let function = run(&program);
    let bound = skeleton_work_upper(&program, function).unwrap();
    let mut exact = ReplayBudget::with_skeleton_limit(bound);
    assert!(
        !hir_skeleton_paths(&program, function, &mut exact)
            .unwrap()
            .is_empty()
    );
    reset_skeleton_materializations();
    let mut short = ReplayBudget {
        remaining: bound - 1,
        skeleton_remaining: 0,
        merge_paths: false,
        physical: None,
    };
    assert_eq!(
        reserve_program_skeleton_work(&program, std::iter::once(function), &mut short)
            .unwrap_err()
            .code,
        "SPX-H006"
    );
    assert_eq!(skeleton_materializations(), 0);
    // The unchanged terminal bound is inclusive; no Copy-only exemption.
    path_budget_errors(function, 0, MAX_REPLAY_PATHS).unwrap();
    assert_eq!(
        path_budget_errors(function, 0, MAX_REPLAY_PATHS + 1)
            .unwrap_err()
            .code,
        "SPX-H006"
    );
}
