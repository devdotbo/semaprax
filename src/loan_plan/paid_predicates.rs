//! Borrowed predicates for actual caller-paid planner discovery.
//! The legacy allocation-bearing predicates remain independent oracles.
use super::*;

pub(super) fn string_slot(expression: &ResolvedExpr) -> Option<&Place> {
    let ResolvedExprKind::Call { callee, type_arguments, instance: None, args } = &expression.kind else { return None; };
    let bytes = match callee.as_str() {
        crate::vec_ops::string_slot::ID => false,
        crate::vec_ops::string_slot::BYTES_ID => true,
        _ => return None,
    };
    let [source, index] = args.as_slice() else { return None; };
    if type_arguments.as_slice() != [ResolvedType::String]
        || !crate::vec_ops::resolved_vec_matches(&source.ty, &ResolvedType::String)
        || !matches!(source.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
        || index.ty != ResolvedType::Usize || index.ownership != OwnershipMode::Value
        || expression.ownership != OwnershipMode::Borrow
        || expression.ty != if bytes { ResolvedType::SliceU8 } else { ResolvedType::Str }
    { return None; }
    match &source.kind {
        ResolvedExprKind::Place(place) if place.projections.is_empty() => Some(place),
        _ => None,
    }
}

