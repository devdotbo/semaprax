//! Exact provisional owner identity; HIR scope independently limits its uses.
use crate::hir::{ResolvedFunction, ResolvedType, ValueId};

pub(super) fn root(function: &ResolvedFunction) -> Option<&ValueId> {
    matches!(
        function.return_type,
        ResolvedType::Bytes | ResolvedType::String
    )
    .then_some(&function.result_id)
}
