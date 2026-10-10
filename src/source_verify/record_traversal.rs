//! Source authority for iteration-local record aliases; no aggregate value exists.
use super::{
    binding::{Binding, CheckedValue},
    type_table::TypeTable,
};
use crate::ast::{Expr, ExprKind, ParamMode, Type, TypeDeclarationKind};
use std::collections::HashMap;

#[derive(Clone, Debug)]
pub(super) struct Row {
    pub(super) root: String,
    pub(super) element: Type,
    pub(super) index_inserted: bool,
    pub(super) freeze: super::binding::SourceLoanId,
}

pub(super) fn admitted(types: &TypeTable<'_>, element: &Type) -> bool {
    super::declared_type::copy_record_collection::admitted(types, element)
        || super::declared_type::owned_leaf_collection::runtime_element(types, element)
            && matches!(element, Type::Named { .. })
}

pub(super) fn project(
    expression: &Expr,
    variables: &HashMap<String, Binding>,
    types: &TypeTable<'_>,
) -> Option<(String, CheckedValue)> {
    let ExprKind::Project { base, field, .. } = &expression.kind else {
        return None;
    };
    let ExprKind::Var(name) = &base.kind else {
        return None;
    };
    let row = variables.get(name)?.traversal_row.as_deref()?;
    let Type::Named { name, arguments } = &row.element else {
        return None;
    };
    if !arguments.is_empty() || !admitted(types, &row.element) {
        return None;
    }
    let declaration = types.declaration(name)?;
    let TypeDeclarationKind::Record { fields } = &declaration.kind else {
        return None;
    };
    let selected = fields.iter().find(|selected| selected.name == *field)?;
    let (ty, mode) = match selected.ty {
        Type::String => (Type::Str, ParamMode::Borrow),
        Type::Bytes => (Type::SliceU8, ParamMode::Borrow),
        ref ty if crate::vec_ops::ast_element_is_admitted(ty) => (ty.clone(), ParamMode::Value),
        _ => return None,
    };
    Some((
        row.root.clone(),
        CheckedValue {
            ty,
            mode,
            native_unit: false,
        },
    ))
}

pub(super) fn body_binding_conflict(
    body: &Expr,
    root: &str,
    row: &str,
    index: Option<&str>,
) -> Option<crate::ast::Span> {
    let mut pending = vec![body];
    while let Some(expression) = pending.pop() {
        if let ExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let crate::ast::Statement::Let {
                    name, name_span, ..
                } = statement
                {
                    if name == root || name == row || index == Some(name.as_str()) {
                        return Some(*name_span);
                    }
                }
            }
        }
        let mut position = 0;
        while let Some(child) = expression.child(position) {
            pending.push(child);
            position += 1;
        }
    }
    None
}
