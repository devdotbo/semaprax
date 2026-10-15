//! Complete frozen finish receiver; only its test-private function name changes.
use super::*;
use crate::hir::ids::fixed_operation_tests::{arguments, owner};

fn saved_finish<'p>(
    function: &FunctionExecutionId,
    path: impl Into<ExprPath<'p>>,
    span: Span,
    op: IteratorOp,
    element: ResolvedType,
    args: Vec<ResolvedExpr>,
) -> Result<ResolvedExpr, Diagnostic> {
    if !matches!(args.as_slice(),[value] if value.ty==op.resolved_param_type(&element)&&value.ownership==OwnershipMode::Own)
    {
        return Err(Diagnostic::io(
            "SPX-H006",
            "iterator operation requires its exact owned carrier",
        ));
    }
    Ok(ResolvedExpr {
        id: path.into().expression_id(function),
        ownership: OwnershipMode::Own,
        ty: op.resolved_return_type(&element),
        kind: ResolvedExprKind::Call {
            callee: DeclarationId::new(op.id()),
            type_arguments: vec![element],
            instance: None,
            args,
        },
        span,
    })
}

#[test]
fn full_iterator_finish_keeps_frozen_outcomes_and_all_diagnostic_fields() {
    let owner = owner();
    for op in crate::iterator_ops::ALL {
        for element in [ResolvedType::I64, ResolvedType::Bool] {
            let parameter = crate::hir::ResolvedParam {
                id: crate::hir::ValueId::parameter(&owner, 0),
                name: "value".into(),
                ownership: OwnershipMode::Own,
                ty: op.resolved_param_type(&element),
                span: Span::default(),
            };
            let args = arguments(&owner, &[parameter]);
            let old_args = args.clone();
            let current_args = args.clone();
            crate::hir::ids::fixed_operation_tests::compare_finish_drop(
                || {
                    saved_finish(
                        &owner,
                        "full.drop",
                        Span::default(),
                        op,
                        element.clone(),
                        old_args,
                    )
                },
                || {
                    finish(
                        &owner,
                        "full.drop",
                        Span::default(),
                        op,
                        element.clone(),
                        current_args,
                    )
                },
                op.id(),
            );
            for mutation in 0..4 {
                let mut supplied = args.clone();
                match mutation {
                    1 => {
                        supplied.pop();
                    }
                    2 => supplied[0].ty = ResolvedType::Bytes,
                    3 => supplied[0].ownership = OwnershipMode::Borrow,
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
                    element.clone(),
                    supplied,
                );
                assert_eq!(current, old, "{op:?}, mutation {mutation}");
                assert_eq!(current.is_ok(), mutation == 0);
            }
        }
    }
}
