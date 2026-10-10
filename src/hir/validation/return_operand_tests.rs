//! Return handoff is a one-time owner move, not a loop-call admission bypass.
use super::*;
const SOURCE: &str = r#"module test.return_operand_domain;
@id("allocate") fn allocate()->Bytes {bytes_zeroed(1usize)}
@id("run") fn run(n:i64)->Bytes {
 let outer=bytes_zeroed(1usize);
 let replacement=allocate();
 let mut i=0;
 while i<n {i=i+1; if i==1 {return outer;};0}
 return outer;
}
@id("main") fn main()->i64 {0}
"#;

fn loop_return(function: &ResolvedFunction) -> &ResolvedExpr {
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("body")
    };
    let body = statements
        .iter()
        .find_map(|statement| match statement {
            ResolvedStatement::LexicalWhile { body, .. } => Some(body),
            _ => None,
        })
        .expect("lexical loop");
    let mut pending: Vec<&ResolvedExpr> = vec![body.as_ref()];
    while let Some(expression) = pending.pop() {
        if matches!(
            &expression.kind,
            ResolvedExprKind::LexicalExit {
                kind: crate::ast::LexicalExitKind::Return,
                ..
            }
        ) {
            return expression;
        }
        for index in 0.. {
            let Some(child) = crate::hir::lexical_exit::child(expression, index) else {
                break;
            };
            pending.push(child);
        }
    }
    panic!("loop return")
}

#[test]
fn direct_outer_owner_return_retains_ordinary_source_and_hir_authentication() {
    let source = crate::check(SOURCE, "return-owner.spx").unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    crate::hir::validate(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "run")
        .unwrap();
    let exit = loop_return(function);
    HirValidator::new(&program)
        .unwrap()
        .validate_while_admission(exit)
        .unwrap();
    let projection = crate::graph::to_json(&source).unwrap();
    // The public graph is pretty JSON; compare the exact structural pair in
    // compact JSON rather than requiring adjacent bytes across whitespace.
    let projection =
        serde_json::to_string(&serde_json::from_str::<serde_json::Value>(&projection).unwrap())
            .unwrap();
    assert!(projection.contains(r#""kind":"completion","name":"never""#));
    assert!(!projection.contains(r#""id":"never:lexical-exit:v1","type""#));
    let canonical = crate::format::canonical(&source);
    let roundtrip = crate::check(&canonical, "return-owner-roundtrip.spx").unwrap();
    assert_eq!(
        crate::graph::to_json(&source).unwrap(),
        crate::graph::to_json(&roundtrip).unwrap()
    );
}

#[test]
fn return_operand_cannot_hide_closed_allocation_or_owning_call_from_loop_admission() {
    let source = crate::check(SOURCE, "return-domain.spx").unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "run")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("body")
    };
    for (statement, spelling) in [(0, "bytes_zeroed(1usize)"), (1, "allocate()")] {
        let ResolvedStatement::Let { value: call, .. } = &statements[statement] else {
            panic!("outside allocation")
        };
        let mut forged = loop_return(function).clone();
        let ResolvedExprKind::LexicalExit { value, .. } = &mut forged.kind else {
            panic!("return")
        };
        *value = Some(Box::new(call.clone()));
        let diagnostic = HirValidator::new(&program)
            .unwrap()
            .validate_while_admission(&forged)
            .unwrap_err();
        assert_eq!(diagnostic.code, "SPX-H006");
        let hostile = SOURCE.replacen("return outer;", &format!("return {spelling};"), 1);
        let diagnostics = crate::check(&hostile, "closed-return-operand.spx").unwrap_err();
        assert!(
            diagnostics
                .iter()
                .any(|d| matches!(d.code, "SPX-T252" | "SPX-T267")),
            "{diagnostics:?}"
        );
    }
}

#[test]
fn cached_guarded_loop_exit_before_return_retains_the_moved_outer_owner() {
    const SOURCE: &str = "module test.guarded_return;@id(\"consume\") fn consume(value:string)->i64 {0}@id(\"main\") fn main(flag:bool)->i64 {let value=\"owner\";while true {if flag {let consumed=consume(value);return consumed;};return 0;}consume(value)}";
    let ast = crate::check(SOURCE, "guard-return-source.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    for kind in [
        crate::ast::LexicalExitKind::Break,
        crate::ast::LexicalExitKind::Continue,
    ] {
        let mut forged = program.clone();
        let function = forged
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "main")
            .unwrap();
        let owner = FunctionExecutionId::Monomorphic(function.id.clone());
        let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
            panic!("body")
        };
        let ResolvedStatement::LexicalWhile {
            condition, body, ..
        } = &mut statements[1]
        else {
            panic!("loop")
        };
        let target = crate::hir::ControlTarget::Loop {
            owner,
            site: condition.id.clone(),
        };
        assert!(crate::hir::lexical_exit::returns_only(body));
        let ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
            panic!("loop body")
        };
        let ResolvedStatement::Let { value, .. } = &mut statements[0] else {
            panic!("guard")
        };
        let ResolvedExprKind::If { then_branch, .. } = &mut value.kind else {
            panic!("if")
        };
        let ResolvedExprKind::Block { tail, .. } = &mut then_branch.kind else {
            panic!("branch")
        };
        tail.kind = ResolvedExprKind::LexicalExit {
            kind,
            target,
            value: None,
        };
        assert!(!crate::hir::lexical_exit::returns_only(body));
        let wire = crate::cache_codec::encode(&forged).unwrap();
        let decoded: ResolvedProgram = crate::cache_codec::decode(&wire).unwrap();
        let refusal = crate::hir::validate(&decoded).unwrap_err();
        assert_eq!(refusal.code, "SPX-H006");
        assert_eq!(
            refusal.message,
            "while loop body changes ownership liveness"
        );
    }
}

#[test]
fn nested_loop_exit_is_local_but_foreign_loop_target_is_not_a_function_return() {
    let ast = crate::check("module nested;@id(\"main\") fn main()->string {let value=\"owner\";while true {while true {break;}return value;}value}", "nested-local-exit.spx").unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "main")
        .unwrap();
    let ResolvedExprKind::Block { statements, .. } = &function.body.kind else {
        panic!("body")
    };
    let ResolvedStatement::LexicalWhile {
        condition, body, ..
    } = &statements[1]
    else {
        panic!("outer loop")
    };
    assert!(crate::hir::lexical_exit::returns_only(body));
    let mut changed = body.as_ref().clone();
    let ResolvedExprKind::Block { statements, .. } = &mut changed.kind else {
        panic!("outer body")
    };
    let ResolvedStatement::LexicalWhile { body, .. } = &mut statements[0] else {
        panic!("inner loop")
    };
    let ResolvedExprKind::Block { tail, .. } = &mut body.kind else {
        panic!("inner body")
    };
    let ResolvedExprKind::LexicalExit { target, .. } = &mut tail.kind else {
        panic!("local exit")
    };
    *target = crate::hir::ControlTarget::Loop {
        owner: FunctionExecutionId::Monomorphic(function.id.clone()),
        site: condition.id.clone(),
    };
    assert!(!crate::hir::lexical_exit::returns_only(&changed));
}
