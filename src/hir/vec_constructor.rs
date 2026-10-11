//! Independent typed constructor authentication. No new runtime operation.
use super::{ResolvedExpr, ResolvedExprKind};

/// The descriptor fields are source provenance, never a supplied cleanup grant.
pub(crate) fn value(expression: &ResolvedExpr) -> Option<&ResolvedExpr> {
    match &expression.kind {
        ResolvedExprKind::VecConstructor { value, .. } => Some(value),
        _ => None,
    }
}
