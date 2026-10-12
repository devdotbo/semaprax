//! Source admission of borrowed field uniqueness. The field literal is static
//! metadata and the named Vec root is the only runtime operand.
use super::binding::{Availability, Binding};
use super::type_table::TypeTable;
use crate::ast::{Expr, ExprKind, ParamMode, Type};
use std::collections::HashMap;
pub(super) fn descriptor(
    types: &TypeTable<'_>,
    type_arguments: &[Type],
    args: &[Expr],
    variables: &HashMap<String, Binding>,
) -> Option<String> {
    let [Type::Named { name, arguments }] = type_arguments else {
        return None;
    };
    let [source, field] = args else {
        return None;
    };
    let (ExprKind::Var(root), ExprKind::String(value)) = (&source.kind, &field.kind) else {
        return None;
    };
    let binding = variables.get(root)?;
    let exact_root = matches!(&binding.ty, Type::Named { name: vec, arguments: element }
        if vec == "Vec"
            && matches!(element.as_slice(), [Type::Named { name: row, arguments }]
                if row == name && arguments.is_empty()));
    if !arguments.is_empty()
        || !exact_root
        || !matches!(binding.mode, ParamMode::Own | ParamMode::Borrow)
        || binding.availability != Availability::Available
        || !binding.moved_places.is_empty()
        || !binding.definitely_partial.is_empty()
    {
        return None;
    }
    crate::vec_ops::field_unique::source(types.declaration(name)?, value)?.identity()
}
