//! Bounded borrowed lookup of certificate anchors; no reconstructed ownership authority.
use super::*;
use crate::hir::ResolvedProgram;
const MAX_DEPTH: usize = 256;
const MAX_WORK: usize = 1_000_000;

fn walk<'a>(
    root: &'a ResolvedExpr,
    inspect: impl FnMut(&'a ResolvedExpr) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    walk_with_limit(root, MAX_WORK, inspect).map(|_| ())
}

fn walk_with_limit<'a>(
    root: &'a ResolvedExpr,
    limit: usize,
    mut inspect: impl FnMut(&'a ResolvedExpr) -> Result<(), Diagnostic>,
) -> Result<usize, Diagnostic> {
    let mut stack: [Option<hir::lexical_exit::ChildCursor<'a>>; MAX_DEPTH] = [None; MAX_DEPTH];
    stack[0] = Some(hir::lexical_exit::ChildCursor::new(root));
    let mut depth = 1;
    let mut work = 0usize;
    charge_inspection_with_limit(root, &mut work, limit)?;
    inspect(root)?;
    while depth != 0 {
        // The saved cursor advances each statement/arm once. Its variable-width
        // scans were paid at entry; each child probe, including exhaustion, is
        // paid here before traversal. No prefix or lexical scope is rebuilt.
        charge_lookup(&mut work, 1, limit)?;
        match stack[depth - 1].as_mut().unwrap().next() {
            None => {
                stack[depth - 1] = None;
                depth -= 1;
            }
            Some(next) => {
                if depth == MAX_DEPTH {
                    return Err(backend_error(
                        "native lexical anchor lookup exceeds depth bound",
                    ));
                }
                charge_inspection_with_limit(next, &mut work, limit)?;
                inspect(next)?;
                stack[depth] = Some(hir::lexical_exit::ChildCursor::new(next));
                depth += 1;
            }
        }
    }
    Ok(work)
}
fn charge_lookup(work: &mut usize, amount: usize, limit: usize) -> Result<(), Diagnostic> {
    *work = work
        .checked_add(amount)
        .filter(|value| *value <= limit)
        .ok_or_else(|| backend_error("native lexical anchor lookup exceeds work bound"))?;
    Ok(())
}
fn charge_inspection(expression: &ResolvedExpr, work: &mut usize) -> Result<(), Diagnostic> {
    // Tail-chain inspection has no child cursor and retains its old debit.
    let width = match &expression.kind {
        ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } => statements.len(),
        _ => 0,
    };
    *work = work
        .checked_add(width)
        .and_then(|n| n.checked_add(1))
        .filter(|n| *n <= MAX_WORK)
        .ok_or_else(|| backend_error("native lexical anchor inspection exceeds work bound"))?;
    Ok(())
}
fn charge_inspection_with_limit(
    expression: &ResolvedExpr,
    work: &mut usize,
    limit: usize,
) -> Result<(), Diagnostic> {
    let width = match &expression.kind {
        ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } => statements.len(),
        ResolvedExprKind::Match { arms, .. } => arms.len(),
        _ => 0,
    };
    // A Block's inspector may enumerate assignments as well as the cursor's
    // statement scan. Pay both passes before either; Match has one arm scan.
    let width = if matches!(
        &expression.kind,
        ResolvedExprKind::Block { .. } | ResolvedExprKind::VecConstructor { .. }
    ) {
        width
            .checked_mul(2)
            .ok_or_else(|| backend_error("native lexical anchor inspection exceeds work bound"))?
    } else {
        width
    };
    *work = work
        .checked_add(width)
        .and_then(|value| value.checked_add(1))
        .filter(|value| *value <= limit)
        .ok_or_else(|| backend_error("native lexical anchor inspection exceeds work bound"))?;
    Ok(())
}
pub(super) fn expression<'a>(
    root: &'a ResolvedExpr,
    site: &ExpressionId,
) -> Result<&'a ResolvedExpr, Diagnostic> {
    let mut found = None;
    walk(root, |expression| {
        if &expression.id == site && found.replace(expression).is_some() {
            return Err(backend_error("native lexical anchor is ambiguous"));
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
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
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
        if &tail.id == next
            && (arm.guard.is_some() || selected.replace((arm, blocks, count)).is_some())
        {
            return Err(backend_error(
                "native iterator latch tail is ambiguous or guarded",
            ));
        }
    }
    selected.ok_or_else(|| backend_error("native iterator latch is not an exact arm tail"))
}

pub(super) fn owning_wrapper<'a>(
    program: &ResolvedProgram,
    root: &'a ResolvedExpr,
    condition: &ExpressionId,
) -> Result<&'a ResolvedExpr, Diagnostic> {
    let mut found = None;
    walk(root, |expression| {
        if let ResolvedExprKind::Block { statements, .. }
        | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
        {
            if matches!(statements.last(), Some(ResolvedStatement::LexicalWhile { condition: candidate, .. }) if &candidate.id == condition)
                && (found.replace(expression).is_some()
                    || hir::lexical_exit::source_for_wrapper(program, expression)?.is_none())
            {
                return Err(backend_error(
                    "native owning Break wrapper is ambiguous or unauthenticated",
                ));
            }
        }
        Ok(())
    })?;
    found.ok_or_else(|| backend_error("native owning Break wrapper is absent"))
}

#[cfg(test)]
mod tests;
