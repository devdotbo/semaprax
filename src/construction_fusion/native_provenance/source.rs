//! Exact source expression comparisons; spans alone never supply carrier authority.
use super::*;
#[derive(Clone, Copy)]
struct Frame<'a> {
    expression: &'a ast::Expr,
    next: usize,
    body: Option<&'a ast::Expr>,
}
#[derive(Clone, Copy)]
pub(super) struct Node<'a> {
    pub(super) expression: &'a ast::Expr,
    body: Option<&'a ast::Expr>,
}
impl std::ops::Deref for Node<'_> {
    type Target = ast::Expr;
    fn deref(&self) -> &Self::Target {
        self.expression
    }
}
pub(super) fn inventory<'a>(source: &'a ast::Program, work: &mut usize) -> Option<Vec<Node<'a>>> {
    let function = source.functions.first()?;
    let bytes = size_of::<[Option<Frame<'_>>; DEPTH]>();
    if !crate::bounded_output::reserve_active_required(bytes) {
        return None;
    }
    let mut frames = [None; DEPTH];
    let mut count = 0usize;
    let mut visit = |emit: &mut dyn FnMut(Node<'a>) -> Option<()>| -> Option<()> {
        frames[0] = Some(Frame {
            expression: &function.body,
            next: 0,
            body: None,
        });
        let mut len = 1;
        while len != 0 {
            let top = frames[len - 1].as_mut()?;
            debit(work, top.next.checked_add(1)?)?;
            if let ast::ExprKind::Block { statements, .. } = &top.expression.kind {
                debit(work, statements.len())?;
            }
            if top.next == 0 {
                emit(Node {
                    expression: top.expression,
                    body: top.body,
                })?;
            }
            if let Some(child) = top.expression.child(top.next) {
                top.next += 1;
                let mut body = top.body;
                if let ast::ExprKind::Block { statements, .. } = &top.expression.kind {
                    // The existing full width debit also owns this borrowed statement scan.
                    for statement in statements {
                        if let ast::Statement::ForBorrow { body: authored, .. } = statement {
                            if std::ptr::eq(child, authored.as_ref()) {
                                body = Some(child);
                            }
                        }
                    }
                }
                *frames.get_mut(len)? = Some(Frame {
                    expression: child,
                    next: 0,
                    body,
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
    if !crate::bounded_output::reserve_active_required(count.checked_mul(size_of::<Node<'_>>())?) {
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
    sources: &[Node<'a>],
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
            result = Some(e.expression);
        }
    }
    result
}
pub(super) fn variable(
    sources: &[Node<'_>],
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
    sources: &[Node<'_>],
    span: ast::Span,
    name: &str,
    bytes: bool,
    traversal: Traversal<'_>,
    work: &mut usize,
) -> Option<()> {
    debit(work, sources.len())?;
    let mut matched = sources.iter().filter(|node| node.span == span);
    let node = matched.next()?;
    if matched.next().is_some()
        || !node
            .body
            .is_some_and(|body| std::ptr::eq(body, traversal.body))
    {
        return None;
    }
    let mut e = node.expression;
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
    let ast::ExprKind::Var(item) = &base.kind else {
        return None;
    };
    if item != traversal.item
        || span.start < traversal.body.span.start
        || span.end > traversal.body.span.end
    {
        return None;
    }
    debit(work, field.len().checked_add(item.len())?)?;
    (field == name).then_some(())
}
pub(super) fn str_bytes(
    sources: &[Node<'_>],
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
