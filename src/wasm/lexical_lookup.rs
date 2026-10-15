//! Identity lookup is bounded and never infers authority from generated names.
use super::*;
use crate::hir::{lexical_exit::ChildCursor, ExpressionId};

pub(super) fn assignment<'a>(
    body: &'a ResolvedExpr,
    id: &ExpressionId,
) -> Result<(&'a crate::hir::ResolvedBinding, &'a ResolvedExpr), Diagnostic> {
    let mut work = crate::cache_codec::MAX_NODES;
    assignment_bounded(body, id, &mut work)
}

fn assignment_bounded<'a>(
    body: &'a ResolvedExpr,
    id: &ExpressionId,
    work: &mut usize,
) -> Result<(&'a crate::hir::ResolvedBinding, &'a ResolvedExpr), Diagnostic> {
    const DEPTH: usize = crate::cache_codec::MAX_DEPTH;
    *work = work
        .checked_sub(DEPTH)
        .ok_or_else(|| error("latch lookup work exhausted"))?;
    if !crate::bounded_output::reserve_active(
        std::mem::size_of::<[Option<ChildCursor<'_>>; DEPTH]>(),
    ) {
        return Err(error("latch lookup backing refused"));
    }
    let mut frames: [Option<ChildCursor<'_>>; DEPTH] = std::array::from_fn(|_| None);
    frames[0] = Some(ChildCursor::new(body));
    let mut used = 1usize;
    let mut found = None;
    while used != 0 {
        *work = work
            .checked_sub(1)
            .ok_or_else(|| error("latch lookup work exhausted"))?;
        let cursor = frames[used - 1].as_mut().expect("active lookup frame");
        if !cursor.is_started() {
            if let ResolvedExprKind::Block { statements, .. }
            | ResolvedExprKind::VecConstructor { statements, .. } = &cursor.expression().kind
            {
                // Charge each statement before inspecting it. This also funds
                // the cursor's once-only advance past statement boundaries,
                // including statements without expression children.
                for statement in statements {
                    *work = work
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
        match cursor.next() {
            Some(child) => {
                if used == frames.len() {
                    return Err(error("latch lookup depth exceeded"));
                }
                frames[used] = Some(ChildCursor::new(child));
                used += 1;
            }
            None => {
                frames[used - 1] = None;
                used -= 1;
            }
        }
    }
    found.ok_or_else(|| error("latch assignment is absent"))
}
fn error(detail: &str) -> Diagnostic {
    Diagnostic::io("SPX-W110", format!("lexical lowering: {detail}"))
}

#[cfg(test)]
#[path = "lexical_lookup/tests.rs"]
mod tests;
