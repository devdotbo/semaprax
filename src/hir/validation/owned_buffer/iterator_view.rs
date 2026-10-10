//! Read-only inline views under independently recovered iterator authority.
//! This lends no named alias or escaping lifetime. The current recognized
//! iteration supplies the owner; finite scratch also proves its Len/Get use. Ordinary
//! typed place/loan validation still reconstructs its live owner and extent.
use super::*;

pub(in crate::hir::validation) fn iterator_byte_view_is_admitted(
    expression: &ResolvedExpr,
    owned_item: Option<&crate::hir::ResolvedBinding>,
    finite_scratch: Option<&crate::finite_byte_scratch::ResolvedScratch<'_>>,
) -> bool {
    expression.ty == ResolvedType::SliceU8
        && expression.ownership == OwnershipMode::Borrow
        && matches!(&expression.kind, ResolvedExprKind::BorrowPlace { operation, place }
            if operation.as_str() == crate::byte_ops::BYTES_AS_SLICE_ID
                && place.projections.is_empty()
                && (owned_item.is_some_and(|item| item.ty == ResolvedType::Bytes
                    && item.ownership == OwnershipMode::Own && item.id == place.root)
                    || finite_scratch.is_some_and(|proof| proof.borrowed_root(&place.root))))
}

#[cfg(test)]
mod tests;
