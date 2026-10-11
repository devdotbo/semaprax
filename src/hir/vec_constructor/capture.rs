//! Borrowed, charged internal-owner inventory; original items cannot observe generated owners.
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedStatement};
pub(super) fn reject_item_capture(
    expansion: &super::Expansion<'_>,
) -> Result<(), crate::diagnostic::Diagnostic> {
    let reject =
        || crate::hir::hir_error("resolved Vec constructor item captures a generated owner");
    let slots = expansion.statements.len();
    let bytes = slots
        .checked_mul(std::mem::size_of::<&crate::hir::ValueId>())
        .ok_or_else(reject)?;
    if !crate::bounded_output::reserve_active_required(bytes) {
        return Err(reject());
    }
    let mut owners = Vec::new();
    owners.try_reserve_exact(slots).map_err(|_| reject())?;
    if owners.capacity() > slots {
        return Err(reject());
    }
    for statement in expansion.statements {
        let ResolvedStatement::Let { binding, .. } = statement else {
            return Err(reject());
        };
        owners.push(&binding.id);
    }
    let mut comparison_work = 0usize;
    let mut node_work = 0usize;
    sort(&mut owners, &mut comparison_work)?;
    fn inspect(
        expression: &ResolvedExpr,
        owners: &[&crate::hir::ValueId],
        depth: usize,
        node_work: &mut usize,
        comparison_work: &mut usize,
    ) -> Result<(), crate::diagnostic::Diagnostic> {
        *node_work = node_work
            .checked_add(1)
            .filter(|work| *work <= 16 * 1024 * 1024)
            .ok_or_else(|| {
                crate::hir::hir_error(
                    "Vec constructor item census exceeds ordinary source-work upper",
                )
            })?;
        if depth > 512 {
            return Err(crate::hir::hir_error(
                "Vec constructor item census exceeds ordinary semantic depth",
            ));
        }
        let place = match &expression.kind {
            ResolvedExprKind::Place(place) | ResolvedExprKind::BorrowPlace { place, .. } => {
                Some(place)
            }
            _ => None,
        };
        if let Some(place) = place {
            if contains(owners, &place.root, comparison_work)? {
                return Err(crate::hir::hir_error(
                    "resolved Vec constructor item captures a generated owner",
                ));
            }
        }
        for child in crate::hir::lexical_exit::ChildCursor::new(expression) {
            inspect(child, owners, depth + 1, node_work, comparison_work)?;
        }
        Ok(())
    }
    for statement in &expansion.statements[1..] {
        let ResolvedStatement::Let { value, .. } = statement else {
            return Err(reject());
        };
        let ResolvedExprKind::Call { args, .. } = &value.kind else {
            return Err(reject());
        };
        inspect(&args[1], &owners, 1, &mut node_work, &mut comparison_work)?;
    }
    Ok(())
}

fn compare(
    left: &crate::hir::ValueId,
    right: &crate::hir::ValueId,
    work: &mut usize,
) -> Result<std::cmp::Ordering, crate::diagnostic::Diagnostic> {
    *work = work
        .checked_add(left.as_str().len())
        .and_then(|n| n.checked_add(right.as_str().len()))
        .filter(|n| *n <= crate::workspace_graph::MAX_BUILDER_BYTES)
        .ok_or_else(|| {
            crate::hir::hir_error("Vec constructor owner comparison exceeds bounded work")
        })?;
    Ok(left.cmp(right))
}
fn contains(
    values: &[&crate::hir::ValueId],
    key: &crate::hir::ValueId,
    work: &mut usize,
) -> Result<bool, crate::diagnostic::Diagnostic> {
    let (mut left, mut right) = (0, values.len());
    while left < right {
        let middle = left + (right - left) / 2;
        match compare(values[middle], key, work)? {
            std::cmp::Ordering::Less => left = middle + 1,
            std::cmp::Ordering::Greater => right = middle,
            std::cmp::Ordering::Equal => return Ok(true),
        }
    }
    Ok(false)
}
fn sort(
    values: &mut [&crate::hir::ValueId],
    work: &mut usize,
) -> Result<(), crate::diagnostic::Diagnostic> {
    fn sift(
        values: &mut [&crate::hir::ValueId],
        mut root: usize,
        end: usize,
        work: &mut usize,
    ) -> Result<(), crate::diagnostic::Diagnostic> {
        loop {
            let left = root * 2 + 1;
            if left >= end {
                return Ok(());
            }
            let mut child = left;
            if left + 1 < end && compare(values[left], values[left + 1], work)?.is_lt() {
                child = left + 1;
            }
            if !compare(values[root], values[child], work)?.is_lt() {
                return Ok(());
            }
            values.swap(root, child);
            root = child;
        }
    }
    for root in (0..values.len() / 2).rev() {
        sift(values, root, values.len(), work)?;
    }
    for end in (1..values.len()).rev() {
        values.swap(0, end);
        sift(values, 0, end, work)?;
    }
    Ok(())
}
