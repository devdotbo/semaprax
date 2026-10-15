//! Never is only authenticated lexical reachability, never an ABI scalar.
use super::*;
const SOURCE: &str = r#"module scalar.completion;
@id("main") fn main()->i64 {let mut i:i64=0;while i<4 {i=i+1;if i==2 {continue;}else{0} if i==3 {break;}else{0} 0} 4}
"#;
fn fixture() -> ResolvedProgram {
    hir::resolve(&crate::check(SOURCE, "completion.spx").unwrap()).unwrap()
}
fn exit(expression: &mut ResolvedExpr) -> Option<&mut ResolvedExpr> {
    if matches!(expression.kind, ResolvedExprKind::LexicalExit { .. }) {
        return Some(expression);
    }
    match &mut expression.kind {
        ResolvedExprKind::Block { statements, tail } => {
            for statement in statements {
                let child = match statement {
                    ResolvedStatement::Let { value, .. }
                    | ResolvedStatement::Assign { value, .. } => value,
                    ResolvedStatement::LexicalWhile { body, .. }
                    | ResolvedStatement::While { body, .. } => body,
                    _ => continue,
                };
                if let Some(found) = exit(child) {
                    return Some(found);
                }
            }
            exit(tail)
        }
        ResolvedExprKind::If {
            then_branch,
            else_branch,
            ..
        } => exit(then_branch).or_else(|| exit(else_branch)),
        _ => None,
    }
}
#[test]
fn authenticated_value_never_completions_prepare_without_widening_scalar_abi_or_storage() {
    let program = fixture();
    hir::validate(&program).unwrap();
    assert!(program.functions[0].cleanup_plan.lexical_control.is_some());
    let plans = prepare(&program, &["main".into()]).unwrap();
    assert_eq!(plans.len(), 1);
    let ast = crate::check(SOURCE, "completion.spx").unwrap();
    wasmparser::Validator::new()
        .validate_all(&crate::wasm::emit_module(&ast).unwrap())
        .unwrap();
    assert!(scalar_type(&ResolvedType::Never).is_none());
    for mode in 0..5 {
        let mut forged = fixture();
        match mode {
            0 => {
                let node = exit(&mut forged.functions[0].body).unwrap();
                node.kind = ResolvedExprKind::Int(0);
            }
            1 => exit(&mut forged.functions[0].body).unwrap().ownership = OwnershipMode::Own,
            2 => exit(&mut forged.functions[0].body).unwrap().ownership = OwnershipMode::Borrow,
            3 => forged.functions[0].return_type = ResolvedType::Never,
            _ => {
                let ResolvedExprKind::Block { statements, .. } = &mut forged.functions[0].body.kind
                else {
                    panic!("block")
                };
                let ResolvedStatement::Let { binding, .. } = &mut statements[0] else {
                    panic!("binding")
                };
                binding.ty = ResolvedType::Never;
            }
        }
        assert_eq!(hir::validate(&forged).unwrap_err().code, "SPX-H006");
        assert_eq!(
            prepare(&forged, &["main".into()]).unwrap_err().code,
            "SPX-H006"
        );
    }
}
