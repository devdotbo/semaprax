//! Full old recursive diagnostics/state, and the narrow removed prefix clone boundary.
use super::*;
use crate::test_allocation_observer::measure;

fn program() -> ResolvedProgram {
    let mut source=String::from("module validation.prefix; @id(\"prefix.add\") fn add(a:i64,b:i64)->i64 {a+b} @id(\"owner:é.λ\") fn exercise(flag:bool)->i64 {\n");
    for index in 0..101 {
        source.push_str(&format!("let v{index}=add({index},1);\n"));
    }
    source.push_str(
        "if flag {v9+v10+v99+v100} else {v0} } @id(\"app.main\") fn main()->i64 {exercise(true)}",
    );
    crate::hir::resolve(
        &crate::parse(&source, std::path::Path::new("validator-prefix.spx")).unwrap(),
    )
    .unwrap()
}
fn compare_path(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    path: &str,
) -> Result<(), Diagnostic> {
    let execution = FunctionExecutionId::Monomorphic(function.id.clone());
    let mut iterative = HirValidator::new(program).unwrap();
    let mut scope = BTreeMap::new();
    for parameter in &function.params {
        scope.insert(
            parameter.id.clone(),
            ValidationBinding {
                contract_local: false,
                mutable: false,
                ty: parameter.ty.clone(),
                ownership: parameter.ownership,
                availability: Availability::Available,
                active_loans: BTreeSet::new(),
                moved_places: BTreeMap::new(),
                definitely_partial: BTreeSet::new(),
            },
        );
        if parameter.ty == ResolvedType::SliceU8 {
            iterative.byte_slice_aliases.insert(
                parameter.id.clone(),
                Place {
                    root: parameter.id.clone(),
                    projections: Vec::new(),
                },
            );
        }
    }
    let mut recursive = iterative.clone();
    let mut recursive_scope = scope.clone();
    let effects = function.effects.iter().cloned().collect();
    let reference = recursive.validate_expr_recursive_reference(
        &execution,
        expression,
        &mut recursive_scope,
        path,
        true,
        Some(&effects),
    );
    let actual = iterative.validate_expr_iterative(
        &execution,
        expression,
        &mut scope,
        path,
        true,
        Some(&effects),
    );
    HirValidator::assert_validation_oracle(
        &actual,
        &reference,
        &iterative,
        &recursive,
        &scope,
        &recursive_scope,
        path,
    );
    actual
}

fn compare_expression(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
) -> Result<(), Diagnostic> {
    compare_path(program, function, expression, "body")
}

#[test]
fn borrowed_continuation_prefixes_preserve_recursive_bytes_header_priority_and_scopes() {
    let program = program();
    crate::hir::validate(&program).unwrap();
    for function in &program.functions {
        compare_expression(&program, function, &function.body).unwrap();
    }
    let function = program
        .functions
        .iter()
        .find(|f| f.id.as_str() == "owner:é.λ")
        .unwrap();
    let owner = FunctionExecutionId::Monomorphic(function.id.clone());
    for root in ["", "body:é.λ", "requires:é.λ.0"] {
        let mut expression = function.body.clone();
        expression.kind = ResolvedExprKind::Int(0);
        expression.ty = ResolvedType::I64;
        expression.ownership = OwnershipMode::Value;
        expression.id = ExpressionId::new(&owner, root);
        compare_path(&program, function, &expression, root).unwrap();
    }

    for forged in ["foreign", "body:é.λ", "body\0"] {
        let mut expression = function.body.clone();
        expression.id = ExpressionId::new(&owner, forged);
        expression.ty = ResolvedType::Never; // later type fault must not win
        let error = compare_expression(&program, function, &expression).unwrap_err();
        assert!(error.message.contains(if forged.contains('\0') {
            "NUL"
        } else {
            "non-canonical identity"
        }));
    }
    let mut expression = function.body.clone();
    expression.id = ExpressionId::new(
        &FunctionExecutionId::Monomorphic(DeclarationId::new("foreign.owner")),
        "body",
    );
    assert!(compare_expression(&program, function, &expression)
        .unwrap_err()
        .message
        .contains("non-canonical identity"));
    let mut expression = function.body.clone();
    let ResolvedExprKind::Block { statements, .. } = &mut expression.kind else {
        panic!("source block")
    };
    statements.swap(9, 10);
    assert!(compare_expression(&program, function, &expression)
        .unwrap_err()
        .message
        .contains("non-canonical identity"));
    let mut expression = function.body.clone();
    let ResolvedExprKind::Block { tail, .. } = &mut expression.kind else {
        panic!("source block")
    };
    let ResolvedExprKind::If { else_branch, .. } = &mut tail.kind else {
        panic!("source sibling branches")
    };
    else_branch.ownership = OwnershipMode::Borrow;
    assert!(compare_expression(&program, function, &expression).is_err());
    // A complete bad length prefix remains a header failure, never a suffix grant.
    let mut expression = function.body.clone();
    expression.id = ExpressionId::from_owned(
        expression
            .id
            .as_str()
            .replace(":expression:4:body", ":expression:04:body"),
    );
    assert_ne!(expression.id, function.body.id);
    assert!(compare_expression(&program, function, &expression)
        .unwrap_err()
        .message
        .contains("non-canonical identity"));
}

#[test]
fn borrowed_authenticated_prefixes_remove_actual_continuation_clone_payload_through_drop() {
    let program = program();
    let template = &program.functions[0].body;
    for owner in [
        FunctionExecutionId::Monomorphic(DeclarationId::new("owner:é.λ")),
        FunctionExecutionId::Generic(FunctionInstanceId("owner:é.λ:instance".to_owned())),
    ] {
        for expected in [
            "",
            "body",
            "contract:é.λ.requires.0",
            "body.s9.value",
            "body.s10.value",
            "body.s99.value",
            "body.s100.value",
        ] {
            let mut expression = template.clone();
            expression.kind = ResolvedExprKind::Int(0);
            expression.ty = ResolvedType::I64;
            expression.ownership = OwnershipMode::Value;
            expression.id = ExpressionId::new(&owner, expected);
            assert!(expression.id.matches(&owner, expected));
            // Equal text with detached Arc backing is legitimate identity text,
            // but only the exact current expression's backing supplies this borrow.
            let detached = ExpressionId::from_owned(expression.id.as_str().to_owned());
            assert_eq!(expression.id, detached);
            for count in [1, 32, 1000] {
                let (_, borrowed) = measure(|| {
                    for _ in 0..count {
                        assert!(expression.id.matches(&owner, expected));
                        let prefix = after_header(&expression, expected.len());
                        assert_eq!(prefix, expected);
                        assert_eq!(
                            prefix.as_ptr(),
                            expression.id.as_str()[expression.id.as_str().len() - expected.len()..]
                                .as_ptr()
                        );
                    }
                });
                assert!(!borrowed.invalid);
                assert_eq!(borrowed.allocations, 0);
                assert_eq!(borrowed.allocated, 0);
                assert_eq!(borrowed.live, 0);
                let current = expected.to_owned();
                let (_, old) = measure(|| {
                    for _ in 0..count {
                        let continuation = current.clone(); // saved original frame operation
                        assert_eq!(continuation, expected);
                        drop(continuation);
                    }
                });
                assert!(!old.invalid);
                assert_eq!(old.allocated, count * expected.len());
                assert_eq!(old.allocated, old.freed);
                assert_eq!(old.live, 0);
                assert_eq!(old.allocations, if expected.is_empty() { 0 } else { count });
            }
        }
    }
}
