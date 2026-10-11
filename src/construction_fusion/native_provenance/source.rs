//! Exact source expression comparisons; spans alone never supply carrier authority.
use super::*;
#[derive(Clone, Copy)]
struct Frame<'a> {
    expression: &'a ast::Expr,
    next: usize,
}
pub(super) fn inventory<'a>(
    source: &'a ast::Program,
    work: &mut usize,
) -> Option<Vec<&'a ast::Expr>> {
    let function = source.functions.first()?;
    let bytes = size_of::<[Option<Frame<'_>>; DEPTH]>();
    if !crate::bounded_output::reserve_active_required(bytes) {
        return None;
    }
    let mut frames = [None; DEPTH];
    let mut count = 0usize;
    let mut visit = |emit: &mut dyn FnMut(&'a ast::Expr) -> Option<()>| -> Option<()> {
        frames[0] = Some(Frame {
            expression: &function.body,
            next: 0,
        });
        let mut len = 1;
        while len != 0 {
            let top = frames[len - 1].as_mut()?;
            debit(work, top.next.checked_add(1)?)?;
            if let ast::ExprKind::Block { statements, .. } = &top.expression.kind {
                debit(work, statements.len())?;
            }
            if top.next == 0 {
                emit(top.expression)?;
            }
            if let Some(child) = top.expression.child(top.next) {
                top.next += 1;
                *frames.get_mut(len)? = Some(Frame {
                    expression: child,
                    next: 0,
                });
                len += 1;
            } else {
                frames[len - 1] = None;
                len -= 1;
            }
        }
        Some(())
    };
    visit(&mut |_| {
        count = count.checked_add(1)?;
        Some(())
    })?;
    if !crate::bounded_output::reserve_active_required(count.checked_mul(size_of::<&ast::Expr>())?)
    {
        return None;
    }
    let mut expressions = Vec::new();
    expressions.try_reserve_exact(count).ok()?;
    if expressions.capacity() != count {
        return None;
    }
    visit(&mut |e| {
        expressions.push(e);
        Some(())
    })?;
    Some(expressions)
}
fn expression<'a>(
    sources: &[&'a ast::Expr],
    span: ast::Span,
    work: &mut usize,
) -> Option<&'a ast::Expr> {
    let mut result = None;
    for e in sources {
        debit(work, 1)?;
        if e.span == span {
            if result.is_some() {
                return None;
            }
            result = Some(*e);
        }
    }
    result
}
pub(super) fn variable(
    sources: &[&ast::Expr],
    span: ast::Span,
    name: &str,
    work: &mut usize,
) -> Option<()> {
    let ast::ExprKind::Var(actual) = &expression(sources, span, work)?.kind else {
        return None;
    };
    debit(work, actual.len())?;
    (actual == name).then_some(())
}
pub(super) fn field(
    sources: &[&ast::Expr],
    span: ast::Span,
    name: &str,
    bytes: bool,
    work: &mut usize,
) -> Option<()> {
    let mut e = expression(sources, span, work)?;
    if bytes {
        let ast::ExprKind::Call {
            name,
            type_arguments,
            args,
        } = &e.kind
        else {
            return None;
        };
        if name != "str_as_bytes" || !type_arguments.is_empty() {
            return None;
        }
        let [argument] = args.as_slice() else {
            return None;
        };
        e = argument;
    }
    let ast::ExprKind::Project { base, field, .. } = &e.kind else {
        return None;
    };
    if !matches!(base.kind, ast::ExprKind::Var(_)) {
        return None;
    }
    debit(work, field.len())?;
    (field == name).then_some(())
}
pub(super) fn str_bytes(
    sources: &[&ast::Expr],
    span: ast::Span,
    alias: &str,
    work: &mut usize,
) -> Option<()> {
    let ast::ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &expression(sources, span, work)?.kind
    else {
        return None;
    };
    let [argument] = args.as_slice() else {
        return None;
    };
    let ast::ExprKind::Var(actual) = &argument.kind else {
        return None;
    };
    debit(work, actual.len())?;
    (name == "str_as_bytes" && type_arguments.is_empty() && actual == alias).then_some(())
}
