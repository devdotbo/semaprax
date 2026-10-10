//! Closed fresh contract-local owner admission, independently reconstructed.
use super::binding::{Availability, Binding};
use crate::ast::{Expr, ExprKind, ParamMode, Type};
use std::collections::HashMap;

/// Consuming calls may transfer only one actual available named local. No
/// outer/provisional owner, field projection, inline call result or capture.
pub(super) fn local(expr: &Expr, ty: &Type, bindings: &HashMap<String, Binding>) -> bool {
    if !matches!(ty, Type::Bytes | Type::String) {
        return false;
    }
    matches!(&expr.kind, ExprKind::Var(name) if name != "result" && bindings.get(name).is_some_and(|binding|
        binding.contract_local && binding.mode == ParamMode::Own && binding.ty == *ty && binding.availability == Availability::Available))
}

/// Constructors are a closed positive set, after ordinary operation/type and
/// static-capacity validation. A user callee name never establishes freshness.
pub(super) fn fresh(expr: &Expr, ty: &Type, bindings: &HashMap<String, Binding>) -> bool {
    if local(expr, ty, bindings) {
        return true;
    }
    match (&expr.kind, ty) {
        (ExprKind::String(_), Type::String) => true,
        (
            ExprKind::Call {
                name,
                type_arguments,
                args,
            },
            Type::Bytes,
        ) if crate::byte_ops::by_name(name) == Some(crate::byte_ops::ByteOp::Zeroed)
            && type_arguments.is_empty() =>
        {
            matches!(args.as_slice(), [Expr {kind: ExprKind::Usize(n), ..}]
                    if *n <= crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES)
        }
        _ => false,
    }
}

#[cfg(test)]
mod tests;
