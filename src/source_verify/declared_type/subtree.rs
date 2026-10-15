//! Exact source type closure for an owned subtree terminal.
use super::*;
pub(super) fn admitted(root: &Type, child: &Type, types: &TypeTable<'_>) -> bool {
    let monomorphic = |ty: &Type| {
        matches!(ty, Type::Named { name, arguments }
        if arguments.is_empty() && types.declaration(name).is_some_and(|d|
            d.type_parameters.is_empty() && matches!(d.kind, TypeDeclarationKind::Record { .. })))
    };
    monomorphic(root)
        && monomorphic(child)
        && (types.is_nested_owned_byte_record(root) || string_record::runtime_admitted(root, types))
        && (types.is_nested_owned_byte_record(child)
            || string_record::runtime_admitted(child, types))
}
