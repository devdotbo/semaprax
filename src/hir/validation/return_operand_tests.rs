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
