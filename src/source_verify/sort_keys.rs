//! Source admission of compile-time sort selectors; the literal is no operand.
use super::type_table::TypeTable;
use crate::ast::{Expr, ExprKind, Type};
pub(super) fn descriptor(
    types: &TypeTable<'_>,
    type_arguments: &[Type],
    args: &[Expr],
) -> Option<String> {
    let [Type::Named { name, arguments }] = type_arguments else {
        return None;
    };
    let [_, selector] = args else {
        return None;
    };
    if !arguments.is_empty() {
        return None;
    }
    let ExprKind::String(value) = &selector.kind else {
        return None;
    };
    crate::vec_ops::sort_keys::source(types.declaration(name)?, value)?.identity()
}
