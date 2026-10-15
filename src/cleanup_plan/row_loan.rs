//! Row Loan v1 is cleanup-inert. It borrows one exact whole-Vec place and one
//! Copy index place; it owns no storage, status source or transition, and its
//! receiving call's borrowed parameter never transfers. Builder and replay
//! both refuse any other child shape instead of inferring one.
use crate::diagnostic::Diagnostic;
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedType};

/// Exactly [whole Vec place, usize index place]; no projection, no move.
pub(super) fn exact_places(args: &[ResolvedExpr]) -> bool {
    matches!(args, [source, index]
        if matches!(&source.kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
            && matches!(source.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
            && matches!(&index.kind, ResolvedExprKind::Place(place) if place.projections.is_empty())
            && index.ty == ResolvedType::Usize
            && index.ownership == OwnershipMode::Value)
}

pub(super) fn require_places(args: &[ResolvedExpr]) -> Result<(), Diagnostic> {
    if exact_places(args) {
        Ok(())
    } else {
        Err(Diagnostic::io(
            "SPX-H006",
            "cleanup row loan is not exactly a whole Vec place and a usize index place",
        ))
    }
}
