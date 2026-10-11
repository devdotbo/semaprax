//! Independent typed constructor authentication. No new runtime operation.
use super::{ResolvedExpr, ResolvedExprKind, ResolvedStatement, ResolvedType};

pub(crate) struct Expansion<'a> {
    pub(crate) element: &'a ResolvedType,
    pub(crate) count: u32,
    pub(crate) statements: &'a [ResolvedStatement],
    pub(crate) tail: &'a ResolvedExpr,
}
/// The marker occupies the ordinary Block node, not a second owning temporary.
pub(crate) fn expansion(expression: &ResolvedExpr) -> Option<Expansion<'_>> {
    match &expression.kind {
        ResolvedExprKind::VecConstructor {
            element,
            count,
            statements,
            tail,
        } => Some(Expansion {
            element,
            count: *count,
            statements,
            tail,
        }),
        _ => None,
    }
}
