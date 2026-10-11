//! Bind one independently frozen HIR traversal to its current source alias/index.
use super::*;
#[derive(Clone, Copy)]
pub(super) struct Traversal<'a> {
    pub(super) item: &'a str,
    pub(super) body: &'a ast::Expr,
    pub(super) owner: &'a hir::Place,
    pub(super) index: &'a hir::ResolvedBinding,
}
pub(super) fn enter<'a>(
    descriptor: &Descriptor<'a>,
    sources: &[source::Node<'a>],
    scratch: &Scratch<'a>,
    region: &'a E,
    inherited: Option<Traversal<'a>>,
    work: &mut usize,
) -> Option<Option<Traversal<'a>>> {
    let K::Block { statements, .. } = &region.kind else {
        return Some(inherited);
    };
    if !matches!(statements.first(),Some(S::Let{binding,..}) if binding.name=="#for-borrow-length")
    {
        return Some(inherited);
    }
    // This reconstruction includes all exact latch/index/root freeze checks.
    crate::loan_plan::traversal::inspect_borrowed(
        descriptor.program,
        region,
        &descriptor.typed.item.ty,
        work,
    )??;
    let [S::Let { value: length, .. }, S::Let { .. }, loop_statement] = statements.as_slice()
    else {
        return None;
    };
    let K::Call { args, .. } = &length.kind else {
        return None;
    };
    let [owner_expr] = args.as_slice() else {
        return None;
    };
    let K::Place(owner) = &owner_expr.kind else {
        return None;
    };
    let body = match loop_statement {
        S::While { body, .. } | S::LexicalWhile { body, .. } => body,
        _ => return None,
    };
    let K::Block {
        statements: iteration,
        ..
    } = &body.kind
    else {
        return None;
    };
    let [S::Let { binding: index, .. }, S::Let {
        value: authored, ..
    }, S::Assign { .. }] = iteration.as_slice()
    else {
        return None;
    };
    let mut found = None;
    for node in sources {
        debit(work, 1)?;
        let expression = node.expression;
        if let ast::ExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                debit(work, 1)?;
                if let ast::Statement::ForBorrow {
                    item,
                    item_span,
                    index: source_index,
                    values,
                    body,
                    span,
                } = statement
                {
                    if *span != region.span {
                        continue;
                    }
                    if found.is_some() {
                        return None;
                    }
                    let ast::ExprKind::Var(name) = &values.kind else {
                        return None;
                    };
                    let binding = alias(scratch, &owner.root, work)?;
                    if !matches!(binding.value, Value::Vector(_))
                        || binding.binding.name != *name
                        || binding.binding.ty != owner_expr.ty
                        || binding.binding.ownership != owner_expr.ownership
                        || region.span != owner_expr.span
                        || body.span != authored.span
                    {
                        return None;
                    }
                    let index_name = source_index
                        .as_ref()
                        .map_or("#for-row-index", |i| i.name.as_str());
                    debit(
                        work,
                        item.len()
                            .checked_add(name.len())?
                            .checked_add(index_name.len())?,
                    )?;
                    if index.name != index_name || index.span != *item_span {
                        return None;
                    }
                    found = Some(Traversal {
                        item,
                        body,
                        owner,
                        index,
                    });
                }
            }
        }
    }
    Some(Some(found?))
}
