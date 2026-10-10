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

// The scan has its own bounded, charged scratch; neither verifier may rely on
// the other's traversal to authenticate this preflight. Cursor frames retain
// ancestors only, never a whole sibling frontier.
const MAX_SCAN_DEPTH: usize = crate::cache_codec::MAX_DEPTH;
const MAX_SCAN_WORK: usize = crate::cache_codec::MAX_NODES;

pub(super) fn body_binding_conflict(
    body: &Expr,
    root: &str,
    row: &str,
    index: Option<&str>,
) -> Result<Option<crate::ast::Span>, ()> {
    scan(body, root, row, index, MAX_SCAN_DEPTH, MAX_SCAN_WORK)
}

fn scan(
    body: &Expr,
    root: &str,
    row: &str,
    index: Option<&str>,
    max_depth: usize,
    mut work: usize,
) -> Result<Option<crate::ast::Span>, ()> {
    // Fixed scratch has no allocator rounding, reallocation overlap or hidden
    // Vec spare capacity. Charge its full physical storage before using it.
    let scratch_bytes = std::mem::size_of::<[(&Expr, usize); MAX_SCAN_DEPTH]>();
    if max_depth == 0
        || max_depth > MAX_SCAN_DEPTH
        || !crate::bounded_output::reserve_active_required(scratch_bytes)
    {
        return Err(());
    }
    let mut frames = [(body, 0usize); MAX_SCAN_DEPTH];
    let mut depth = 0usize;
    let mut entering = Some(body);
    loop {
        if let Some(expression) = entering.take() {
            work = work.checked_sub(1).ok_or(())?;
            if let ExprKind::Block { statements, .. } = &expression.kind {
                for statement in statements {
                    work = work.checked_sub(1).ok_or(())?;
                    if let crate::ast::Statement::Let {
                        name, name_span, ..
                    } = statement
                    {
                        if name == root || name == row || index == Some(name.as_str()) {
                            return Ok(Some(*name_span));
                        }
                    }
                }
            }
            let mut children = 0usize;
            while child_counted(expression, children, &mut work)?.is_some() {
                children += 1;
            }
            if depth == max_depth {
                return Err(());
            }
            frames[depth] = (expression, children);
            depth += 1;
        }
        let Some(frame) = depth.checked_sub(1) else {
            return Ok(None);
        };
        let (expression, remaining) = frames[frame];
        if remaining == 0 {
            depth -= 1;
        } else {
            frames[frame].1 -= 1;
            // Match the old LIFO frontier's reverse-child visit order exactly.
            entering = child_counted(expression, remaining - 1, &mut work)?;
        }
    }
}

fn child_counted<'a>(
    expression: &'a Expr,
    index: usize,
    work: &mut usize,
) -> Result<Option<&'a Expr>, ()> {
    // Block/Match child lookup scans preceding statements/arms. Debit that
    // lookup work too, including a terminal missing-child probe, before it runs.
    let cost = match expression.kind {
        ExprKind::Block { .. } => index.checked_add(1).ok_or(())?,
        ExprKind::Match { .. } => index
            .checked_add(1)
            .and_then(|n| n.checked_mul(2))
            .ok_or(())?,
        _ => 1,
    };
    *work = work.checked_sub(cost).ok_or(())?;
    Ok(expression.child(index))
}

#[cfg(test)]
mod tests;
