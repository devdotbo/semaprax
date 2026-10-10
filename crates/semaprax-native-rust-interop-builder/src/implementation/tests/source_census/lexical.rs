use super::*;
#[test]
fn lexical_source_refuses_before_legacy_builder_reservation() {
    let source = semaprax::parse(
        "module capacity.lexical; @id(\"app.main\") fn main()->i64 { return 7; }",
        "lexical.spx",
    )
    .unwrap();
    let error = validate_native_rust_source_expression_budget(&source).unwrap_err();
    assert_eq!(error.code, "SPX-B107");
    assert!(error.message.contains("lexical control"));
    let ordinary = semaprax::parse(
        "module capacity.legacy; @id(\"app.main\") fn main()->i64 { 7 }",
        "legacy.spx",
    )
    .unwrap();
    validate_native_rust_source_expression_budget(&ordinary).unwrap();
}

#[test]
fn lexical_hir_census_counts_return_operand_and_target_backing() {
    let checked = semaprax::check(
        "module capacity.legacy; @id(\"app.main\") fn main()->i64 { 7 }",
        "legacy.spx",
    )
    .unwrap();
    let resolved = semaprax::hir::resolve(&checked).unwrap();
    let ordinary = resolved.functions[0].body.clone();
    let ordinary_bytes = hir_expr_owned_capacity(&ordinary).unwrap();
    let mut exit = ordinary.clone();
    exit.ty = ResolvedType::Never;
    exit.kind = ResolvedExprKind::LexicalExit {
        kind: semaprax::ast::LexicalExitKind::Return,
        target: semaprax::hir::ControlTarget::Function(
            semaprax::hir::FunctionExecutionId::Monomorphic(semaprax::hir::DeclarationId::new(
                "capacity.target",
            )),
        ),
        value: Some(Box::new(ordinary)),
    };
    assert_eq!(
        hir_expr_owned_capacity(&exit).unwrap() - ordinary_bytes,
        std::mem::size_of::<ResolvedExpr>()
            + exit.id.owned_allocation_bytes().unwrap()
            + "capacity.target".len()
    );
}
