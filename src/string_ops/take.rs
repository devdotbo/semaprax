//! Exact named-owner transfer facts, independent of attached cleanup metadata.
use crate::ast::{Expr, ExprKind};
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedType};
pub(crate) const ID: &str = "core.string.take";

pub(crate) fn source_operand(args: &[Expr], allow_moves: bool) -> bool {
    allow_moves
        && matches!(
            args,
            [Expr {
                kind: ExprKind::Var(_),
                ..
            }]
        )
}

/// This shape grants no availability or loan verdict. The ordinary checked
/// argument receiver must still authenticate the actual current binding.
pub(crate) fn operand(expression: &ResolvedExpr) -> Option<&ResolvedExpr> {
    let ResolvedExprKind::Call {
        callee,
        instance,
        type_arguments,
        args,
    } = &expression.kind
    else {
        return None;
    };
    if callee.as_str() != ID
        || instance.is_some()
        || !type_arguments.is_empty()
        || expression.ty != ResolvedType::String
        || expression.ownership != OwnershipMode::Own
    {
        return None;
    }
    let [argument] = args.as_slice() else {
        return None;
    };
    (argument.ty == ResolvedType::String
        && argument.ownership == OwnershipMode::Own
        && matches!(&argument.kind, ResolvedExprKind::Place(place) if place.projections.is_empty()))
    .then_some(argument)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) fn operand_with_work<'a, E>(
    expression: &'a ResolvedExpr,
    work: crate::hir::funded::work::Work<'_, E>,
) -> Result<Option<&'a ResolvedExpr>, crate::hir::funded::FundedError<E>> {
    use crate::hir::funded::work as paid;
    paid::tick(work, 1)?;
    let ResolvedExprKind::Call {
        callee,
        instance,
        type_arguments,
        args,
    } = &expression.kind
    else {
        return Ok(None);
    };
    paid::tick(work, paid::str_units(callee.as_str(), ID))?;
    if callee.as_str() != ID {
        return Ok(None);
    }
    // The complete original predicate inspects only these bounded scalar
    // discriminants/lengths. It performs no hidden lookup or tree walk.
    paid::tick(work, 10)?;
    if instance.is_some()
        || !type_arguments.is_empty()
        || expression.ty != ResolvedType::String
        || expression.ownership != OwnershipMode::Own
    {
        return Ok(None);
    }
    let [argument] = args.as_slice() else {
        return Ok(None);
    };
    Ok((argument.ty == ResolvedType::String
        && argument.ownership == OwnershipMode::Own
        && matches!(&argument.kind,ResolvedExprKind::Place(place) if place.projections.is_empty()))
    .then_some(argument))
}
