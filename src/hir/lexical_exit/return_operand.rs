//! A direct named return handoff executes once, rather than restoring a loop
//! entry owner. This is a shape exemption, never call/effect/allocation authority.
//! Ordinary source/HIR expression, ownership and lexical-target validation still
//! authenticate the value and complete function result contract.
use crate::ast::{Expr, ExprKind};
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedType};

pub(crate) fn source_named_handoff(value: &Expr) -> bool {
    matches!(value.kind, ExprKind::Var(_))
}

pub(crate) fn resolved_owner_handoff(value: &ResolvedExpr) -> bool {
    value.ownership == OwnershipMode::Own
        && value.ty != ResolvedType::Never
        && matches!(&value.kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
}
