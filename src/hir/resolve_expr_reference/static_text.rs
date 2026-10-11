//! Preserve the ordinary owned String tuple while the future form stays refused.
use super::*;
pub(super) fn owned_string(value: &str) -> (ResolvedExprKind, ResolvedType, OwnershipMode) {
    (ResolvedExprKind::String(value.to_owned()), ResolvedType::String, OwnershipMode::Own)
}
