//! Complete frozen finish receiver; only its test-private function name changes.
use super::*;
use crate::hir::ids::fixed_operation_tests::{arguments, owner};

fn saved_finish<'p>(
    function: &FunctionExecutionId,
    path: impl Into<ExprPath<'p>>,
    span: Span,
    op: ListOp,
    element: ResolvedType,
    args: Vec<ResolvedExpr>,
) -> Result<ResolvedExpr, Diagnostic> {
    if element != ResolvedType::I64
        || args.len() != op.argument_count()
        || args
            .iter()
            .zip(op.resolved_params())
            .any(|(actual, expected)| {
                actual.ty != expected.ty || actual.ownership != expected.ownership
            })
    {
        return Err(Diagnostic::io(
            "SPX-H006",
            "immutable list operation requires its exact checked arguments",
        ));
    }
    Ok(ResolvedExpr {
        id: path.into().expression_id(function),
        ownership: OwnershipMode::Value,
        ty: op.resolved_return_type(),
        kind: ResolvedExprKind::Call {
            callee: DeclarationId::new(op.id()),
            type_arguments: Vec::new(),
            instance: None,
            args,
        },
        span,
    })
}

#[test]
fn full_list_finish_keeps_frozen_outcomes_and_all_diagnostic_fields() {
    let owner = owner();
    for op in crate::list_ops::ALL {
        let args = arguments(&owner, &op.resolved_params());
        let old_args = args.clone();
        let current_args = args.clone();
        crate::hir::ids::fixed_operation_tests::compare_finish_drop(
            || {
                saved_finish(
                    &owner,
                    "full.drop",
                    Span::default(),
                    op,
                    ResolvedType::I64,
                    old_args,
                )
            },
            || {
                finish(
                    &owner,
                    "full.drop",
                    Span::default(),
                    op,
                    ResolvedType::I64,
                    current_args,
                )
            },
            op.id(),
        );
        for argument_index in 0..args.len().max(1) {
            for mutation in 0..5 {
                let mut supplied = args.clone();
                let mut element = ResolvedType::I64;
                match mutation {
                    1 => supplied.push(
                        arguments(&owner, &crate::list_ops::ListOp::Cons.resolved_params())
                            .remove(0),
                    ),
                    2 if !supplied.is_empty() => supplied[argument_index].ty = ResolvedType::Bytes,
                    3 if !supplied.is_empty() => {
                        supplied[argument_index].ownership =
                            if supplied[argument_index].ownership == OwnershipMode::Own {
                                OwnershipMode::Borrow
                            } else {
                                OwnershipMode::Own
                            }
                    }
                    4 => element = ResolvedType::Bool,
                    _ => {}
                }
                let old = saved_finish(
                    &owner,
                    "λ.call\0tail",
                    Span::default(),
                    op,
                    element.clone(),
                    supplied.clone(),
                );
                let current = finish(
                    &owner,
                    "λ.call\0tail",
                    Span::default(),
                    op,
                    element,
                    supplied,
                );
                assert_eq!(current, old, "{op:?}, mutation {mutation}");
                if mutation == 0 {
                    assert!(current.is_ok());
                }
                if mutation == 1 || mutation == 4 || (matches!(mutation, 2 | 3) && !args.is_empty())
                {
                    assert!(current.is_err());
                }
            }
        }
    }
}
