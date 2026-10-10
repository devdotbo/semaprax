//! Bounded borrowed lookup of certificate anchors; no reconstructed ownership authority.
use super::*;
const MAX_DEPTH: usize = 256;
const MAX_WORK: usize = 1_000_000;

fn walk<'a>(
    root: &'a ResolvedExpr,
    mut inspect: impl FnMut(&'a ResolvedExpr) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let mut stack: [Option<(&ResolvedExpr, usize)>; MAX_DEPTH] = [None; MAX_DEPTH];
    stack[0] = Some((root, 0));
    let mut depth = 1;
    let mut work = 0usize;
    charge_inspection(root, &mut work)?;
    inspect(root)?;
    while depth != 0 {
        let (expression, index) = stack[depth - 1].as_mut().unwrap();
        // child() scans the Block/Match prefix. Debit the full possible scan,
        // including the final unsuccessful lookup, before touching it.
        let scan = match &expression.kind {
            ResolvedExprKind::Block { statements, .. } => statements.len(),
            ResolvedExprKind::Match { arms, .. } => arms.len(),
            _ => 0,
        };
        work = work
            .checked_add(scan + 1)
            .filter(|value| *value <= MAX_WORK)
            .ok_or_else(|| backend_error("native lexical anchor lookup exceeds work bound"))?;
        let next = hir::lexical_exit::child(expression, *index);
        *index += 1;
        match next {
            None => depth -= 1,
            Some(next) => {
                if depth == MAX_DEPTH {
                    return Err(backend_error(
                        "native lexical anchor lookup exceeds depth bound",
                    ));
                }
                charge_inspection(next, &mut work)?;
                inspect(next)?;
                stack[depth] = Some((next, 0));
                depth += 1;
            }
        }
    }
    Ok(())
}
fn charge_inspection(expression: &ResolvedExpr, work: &mut usize) -> Result<(), Diagnostic> {
    let width = match &expression.kind {
        ResolvedExprKind::Block { statements, .. } => statements.len(),
        _ => 0,
    };
    *work = work
        .checked_add(width + 1)
        .filter(|value| *value <= MAX_WORK)
        .ok_or_else(|| backend_error("native lexical anchor inspection exceeds work bound"))?;
    Ok(())
}
pub(super) fn expression<'a>(
    root: &'a ResolvedExpr,
    site: &ExpressionId,
) -> Result<&'a ResolvedExpr, Diagnostic> {
    let mut found = None;
    walk(root, |expression| {
        if &expression.id == site {
            if found.replace(expression).is_some() {
                return Err(backend_error("native lexical anchor is ambiguous"));
            }
        }
        Ok(())
    })?;
    found.ok_or_else(|| backend_error("native lexical anchor is absent"))
}
pub(super) fn assignment<'a>(
    root: &'a ResolvedExpr,
    site: &ExpressionId,
) -> Result<(&'a hir::ResolvedBinding, &'a ResolvedExpr), Diagnostic> {
    let mut found = None;
    walk(root, |expression| {
        if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::Assign {
                    binding,
                    field: None,
                    value,
                    ..
                } = statement
                {
                    if &value.id == site && found.replace((binding, value)).is_some() {
                        return Err(backend_error("native lexical assignment is ambiguous"));
                    }
                }
            }
        }
        Ok(())
    })?;
    found.ok_or_else(|| backend_error("native lexical assignment is absent"))
}

pub(super) fn iterator_tail<'a>(
    join: &'a ResolvedExpr,
    next: &ExpressionId,
) -> Result<
    (
        &'a hir::ResolvedMatchArm,
        [Option<&'a ResolvedExpr>; MAX_DEPTH],
        usize,
    ),
    Diagnostic,
> {
    let ResolvedExprKind::Match {
        mode: hir::ResolvedMatchMode::Own,
        arms,
        ..
    } = &join.kind
    else {
        return Err(backend_error(
            "native iterator latch assignment is not owning Match",
        ));
    };
    let mut selected = None;
    let mut work = 0;
    for arm in arms {
        let mut blocks = [None; MAX_DEPTH];
        let mut count = 0;
        let mut tail = &arm.value;
        while let ResolvedExprKind::Block { tail: inner, .. } = &tail.kind {
            charge_inspection(tail, &mut work)?;
            if count == MAX_DEPTH {
                return Err(backend_error("native iterator tail exceeds depth bound"));
            }
            blocks[count] = Some(tail);
            count += 1;
            tail = inner;
        }
        charge_inspection(tail, &mut work)?;
        if &tail.id == next {
            if arm.guard.is_some() || selected.replace((arm, blocks, count)).is_some() {
                return Err(backend_error(
                    "native iterator latch tail is ambiguous or guarded",
                ));
            }
        }
    }
    selected.ok_or_else(|| backend_error("native iterator latch is not an exact arm tail"))
}
