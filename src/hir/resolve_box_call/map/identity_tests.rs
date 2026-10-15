//! Complete frozen finish receiver; only its test-private function name changes.
use super::*;
use crate::hir::ids::fixed_operation_tests::{arguments, owner};

fn saved_finish<'p>(
    function: &FunctionExecutionId,
    path: impl Into<ExprPath<'p>>,
    span: Span,
    op: MapOp,
    collection: ResolvedType,
    args: Vec<ResolvedExpr>,
) -> Result<ResolvedExpr, Diagnostic> {
    let type_arguments = op
        .resolved_arguments(&collection)
        .ok_or_else(|| Diagnostic::io("SPX-H006", "invalid typed collection instance"))?;
    let (params, ty) = op
        .resolved_signature(&type_arguments)
        .ok_or_else(|| Diagnostic::io("SPX-H006", "invalid typed collection signature"))?;
    if args.len() != params.len()
        || args
            .iter()
            .zip(&params)
            .any(|(arg, param)| arg.ty != param.ty)
    {
        return Err(Diagnostic::io(
            "SPX-H006",
            "typed collection arguments do not match exact signature",
        ));
    }
    let ownership = if crate::map_ops::is_collection(&ty) || ty == ResolvedType::String {
        OwnershipMode::Own
    } else {
        OwnershipMode::Value
    };
    Ok(ResolvedExpr {
        id: path.into().expression_id(function),
        ownership,
        ty,
        span,
        kind: ResolvedExprKind::Call {
            callee: DeclarationId::new(op.id()),
            type_arguments,
            instance: None,
            args,
        },
    })
}

#[test]
fn full_map_finish_keeps_frozen_outcomes_and_all_diagnostic_fields() {
    let owner = owner();
    for op in MapOp::ALL {
        let ast = if op.is_set() {
            vec![Type::I64]
        } else {
            vec![Type::I64, Type::I64]
        };
        let collection = crate::map_ops::ast_resolved(&op.ast_type(&ast).unwrap()).unwrap();
        let types = op.resolved_arguments(&collection).unwrap();
        let (params, _) = op.resolved_signature(&types).unwrap();
        let args = arguments(&owner, &params);
        let old_args = args.clone();
        let current_args = args.clone();
        crate::hir::ids::fixed_operation_tests::compare_finish_drop(
            || {
                saved_finish(
                    &owner,
                    "full.drop",
                    Span::default(),
                    op,
                    collection.clone(),
                    old_args,
                )
            },
            || {
                finish(
                    &owner,
                    "full.drop",
                    Span::default(),
                    op,
                    collection.clone(),
                    current_args,
                )
            },
            op.id(),
        );
        for mutation in 0..5 {
            let mut supplied = args.clone();
            let mut ty = collection.clone();
            match mutation {
                1 => {
                    supplied.pop();
                }
                2 => supplied[0].ty = ResolvedType::Bytes,
                3 => {
                    supplied[0].ownership = if supplied[0].ownership == OwnershipMode::Value {
                        OwnershipMode::Own
                    } else {
                        OwnershipMode::Value
                    }
                }
                4 => ty = ResolvedType::I64,
                _ => {}
            }
            let old = saved_finish(
                &owner,
                "λ.call\0tail",
                Span::default(),
                op,
                ty.clone(),
                supplied.clone(),
            );
            let current = finish(&owner, "λ.call\0tail", Span::default(), op, ty, supplied);
            assert_eq!(current, old, "{op:?}, mutation {mutation}");
            // Original finish checks exact types/arity, not ownership.
            if mutation == 0 || mutation == 3 {
                assert!(current.is_ok());
            }
            if matches!(mutation, 1 | 2 | 4) {
                assert!(current.is_err());
            }
        }
    }
}
