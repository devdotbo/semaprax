//! Identity lookup is bounded and never infers authority from generated names.
use super::*;
use crate::hir::ExpressionId;

pub(super) fn assignment<'a>(
    body: &'a ResolvedExpr,
    id: &ExpressionId,
) -> Result<(&'a crate::hir::ResolvedBinding, &'a ResolvedExpr), Diagnostic> {
    let mut frames = [(body, 0usize); crate::cache_codec::MAX_DEPTH];
    let mut used = 1usize;
    let mut work = crate::cache_codec::MAX_NODES;
    let mut found = None;
    while used != 0 {
        work = work
            .checked_sub(1)
            .ok_or_else(|| error("latch lookup work exhausted"))?;
        let (expression, position) = frames[used - 1];
        if position == 0 {
            if let (ResolvedExprKind::Block { statements, .. }
            | ResolvedExprKind::VecConstructor { statements, .. }) = &expression.kind
            {
                for statement in statements {
                    work = work
                        .checked_sub(1)
                        .ok_or_else(|| error("latch lookup work exhausted"))?;
                    if let ResolvedStatement::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } = statement
                    {
                        if &value.id == id && found.replace((binding, value)).is_some() {
                            return Err(error("latch assignment is duplicated"));
                        }
                    }
                }
            }
        }
        frames[used - 1].1 += 1;
        if matches!(
            expression.kind,
            ResolvedExprKind::Block { .. }
                | ResolvedExprKind::VecConstructor { .. }
                | ResolvedExprKind::Match { .. }
        ) {
            work = work
                .checked_sub(
                    position
                        .checked_add(1)
                        .and_then(|n| n.checked_mul(2))
                        .ok_or_else(|| error("latch work overflow"))?,
                )
                .ok_or_else(|| error("latch lookup work exhausted"))?;
        }
        match crate::hir::lexical_exit::child(expression, position) {
            Some(child) => {
                if used == frames.len() {
                    return Err(error("latch lookup depth exceeded"));
                }
                frames[used] = (child, 0);
                used += 1;
            }
            None => used -= 1,
        }
    }
    found.ok_or_else(|| error("latch assignment is absent"))
}
fn error(detail: &str) -> Diagnostic {
    Diagnostic::io("SPX-W110", format!("lexical lowering: {detail}"))
}
