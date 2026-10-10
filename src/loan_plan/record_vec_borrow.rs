//! Exact record Vec reads borrow the owner, despite the generic prelude's
//! nominal ownership mode. Consuming operations never acquire this exemption.
use super::*;

pub(super) fn argument(
    program: &dyn crate::cleanup::ReplayContext,
    call: &ResolvedExpr,
    position: usize,
    argument: &ResolvedExpr,
) -> bool {
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &call.kind
    else {
        return false;
    };
    let (Some(op), [element]) = (
        crate::vec_ops::by_id(callee.as_str()),
        type_arguments.as_slice(),
    ) else {
        return false;
    };
    let expected_args = match op {
        crate::vec_ops::VecOp::Len | crate::vec_ops::VecOp::Capacity => 1,
        crate::vec_ops::VecOp::Get | crate::vec_ops::VecOp::CloneAt => 2,
        _ => return false,
    };
    position == 0
        && (crate::hir::copy_record_collection::admitted(program.declarations(), element)
            || (matches!(
                op,
                crate::vec_ops::VecOp::Len | crate::vec_ops::VecOp::Capacity
            ) && crate::hir::owned_leaf_collection::layout(program.declarations(), element)
                .is_some_and(|layout| layout.fields.is_some())))
        && op.param_ownership_for(position, element) == OwnershipMode::Borrow
        && op.accepts_resolved(position, &argument.ty, element)
        && matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
        && args.len() == expected_args
        && args
            .iter()
            .enumerate()
            .all(|(index, arg)| op.accepts_resolved(index, &arg.ty, element))
        && call.ty == op.resolved_return_type(element)
}
