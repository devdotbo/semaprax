//! Exact streamed identity, removed child-label allocation, and saved full walkers.
use super::*;
use crate::test_allocation_observer::measure;

#[test]
fn borrowed_labels_match_exact_owned_bytes_without_allocation_or_debit() {
    for parent in [
        "",
        "body",
        "contract:é.λ",
        "body.s9.value",
        "body.s10.value",
        "body.s99.value",
        "body.s100.value",
    ] {
        for index in [0, 9, 10, 99, 100, usize::MAX] {
            for other in [0, 9, 10, usize::MAX] {
                for (suffix, expected) in [
                    (".value", format!("{parent}.value")),
                    (".s{}.value", format!("{parent}.s{index}.value")),
                    (".arg.0.arg.{}", format!("{parent}.arg.0.arg.{index}")),
                    (
                        ".arm.{}.binding.{}",
                        format!("{parent}.arm.{index}.binding.{other}"),
                    ),
                    (
                        ".native-rust-arg.{}",
                        format!("{parent}.native-rust-arg.{index}"),
                    ),
                    (".{}", format!("{parent}.{index}")),
                ] {
                    let label = Label::child(parent, suffix, [index, other]).unwrap();
                    assert_eq!(label.to_string(), expected);
                    assert_eq!(label.len(), expected.len());
                    for owner in [
                        FunctionExecutionId::Monomorphic(DeclarationId::new("owner:é.λ")),
                        FunctionExecutionId::Generic(FunctionInstanceId(
                            "owner:é.λ:instance".to_owned(),
                        )),
                    ] {
                        let expression = ExpressionId::new(&owner, &expected);
                        let local = ValueId::local(&owner, &expected);
                        let forged = ExpressionId::from_owned(format!("{}:", expression));
                        let (matches, overflowed, debit) =
                            crate::bounded_output::with_limit_usage(0, || {
                                expression.matches_display(&owner, label.len(), label)
                                    && local.matches_local_display(&owner, label.len(), label)
                                    && !forged.matches_display(&owner, label.len(), label)
                                    && !expression.matches_display(&owner, label.len() + 1, label)
                            });
                        assert!(matches);
                        assert!(!overflowed);
                        assert_eq!(debit, 0);
                        for count in [1, 32] {
                            let (_, borrowed) = measure(|| {
                                for _ in 0..count {
                                    let current =
                                        Label::child(parent, suffix, [index, other]).unwrap();
                                    assert!(expression.matches_display(
                                        &owner,
                                        current.len(),
                                        current
                                    ));
                                }
                            });
                            assert!(!borrowed.invalid);
                            assert_eq!(borrowed.allocations, 0);
                            assert_eq!(borrowed.allocated, 0);
                            assert_eq!(borrowed.live, 0);
                            let (_, old) = measure(|| {
                                for _ in 0..count {
                                    let current =
                                        scope_checkpoint::paths::owned(&expected).unwrap();
                                    assert!(expression.matches(&owner, &current));
                                    drop(current);
                                }
                            });
                            assert!(!old.invalid);
                            assert_eq!(old.allocations, count);
                            assert_eq!(old.allocated, count * expected.len());
                            assert_eq!(old.allocated, old.freed);
                            assert_eq!(old.live, 0);
                        }
                    }
                }
            }
        }
    }
}

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
    let mut owned = iterative.clone();
    let mut owned_scope = scope.clone();
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
    let owned_result = owned.validate_expr_iterative_owned_oracle(
        &execution,
        expression,
        &mut owned_scope,
        path,
        true,
        Some(&effects),
    );
    HirValidator::assert_validation_oracle(
        &actual,
        &owned_result,
        &iterative,
        &owned,
        &scope,
        &owned_scope,
        path,
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
fn borrowed_child_labels_preserve_both_complete_saved_walkers_and_header_priority() {
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
fn streamed_argument_labels_preserve_all_original_special_prefixes() {
    let program = program();
    let mut expression = program.functions[0].body.clone();
    for kind in [
        ResolvedExprKind::Call {
            callee: DeclarationId::new("ordinary.call"),
            type_arguments: vec![],
            instance: None,
            args: vec![],
        },
        ResolvedExprKind::Call {
            callee: DeclarationId::new(crate::vec_ops::string_slot::BYTES_ID),
            type_arguments: vec![],
            instance: None,
            args: vec![],
        },
        ResolvedExprKind::LiteralFormat {
            template: String::new(),
            args: vec![],
        },
        ResolvedExprKind::VecFieldRead {
            origin: crate::hir::VecFieldReadOrigin::Explicit,
            element: ResolvedType::String,
            field: DeclarationId::new("row.field"),
            bytes: false,
            args: vec![],
        },
        ResolvedExprKind::VecFieldRead {
            origin: crate::hir::VecFieldReadOrigin::Explicit,
            element: ResolvedType::String,
            field: DeclarationId::new("row.field"),
            bytes: true,
            args: vec![],
        },
    ] {
        expression.kind = kind;
        for parent in ["", "body:é.λ", "body.s100.value"] {
            for index in [0, 9, 10, 99, 100] {
                let original_parent = if matches!(
                    &expression.kind,
                    ResolvedExprKind::VecFieldRead { bytes: true, .. }
                ) {
                    format!("{parent}.arg.0")
                } else {
                    parent.to_owned()
                };
                let original = crate::vec_ops::string_slot::argument_path(
                    &expression,
                    &original_parent,
                    index,
                );
                let borrowed = Label::argument(&expression, parent, index).unwrap();
                assert_eq!(borrowed.to_string(), original);
                assert_eq!(borrowed.len(), original.len());
            }
        }
    }
}

#[test]
fn removed_label_buffer_refusal_retains_the_owned_formatter_control() {
    let parent = "body:é.λ";
    let expected = format!("{parent}.arm.100.binding.99");
    let (borrowed, overflowed, used) = crate::bounded_output::with_limit_usage(0, || {
        Label::child(parent, ".arm.{}.binding.{}", [100, 99])
            .unwrap()
            .len()
    });
    assert_eq!(borrowed, expected.len());
    assert!(!overflowed);
    assert_eq!(used, 0);
    let (owned, overflowed, _) =
        crate::bounded_output::with_limit_usage(expected.len() - 1, || {
            scope_checkpoint::paths::owned(&expected)
        });
    assert!(overflowed);
    assert!(owned.is_err());
}
