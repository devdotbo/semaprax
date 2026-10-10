//! Metered primitive renewal reconstruction. The selected entry independently
//! closes Vec/iterator/String-replacement families before using this subset.
use super::*;
use crate::cleanup_plan::{replay_btree_node_upper, ReplayPhysicalReservation};
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedStatement, ValueId};
use std::collections::BTreeSet;
const DEPTH: usize = 514;
const WORK: usize = crate::cache_codec::MAX_NODES;
type Frame<'a> = Option<(&'a ResolvedExpr, usize)>;
fn error() -> crate::diagnostic::Diagnostic {
    crate::diagnostic::Diagnostic::io("SPX-H006", "selected primitive renewal census refused")
}
pub(crate) fn bindings_with_reservation<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    physical: &mut ReplayPhysicalReservation,
) -> Result<RenewalBindings<'a>, crate::diagnostic::Diagnostic> {
    physical
        .reserve(std::mem::size_of::<[Frame<'_>; DEPTH]>())
        .map_err(|_| error())?;
    let mut frames = [None; DEPTH];
    let mut mutable = BTreeSet::new();
    let mut work = WORK;
    walk(function, &mut frames, &mut work, |statement| {
        if let ResolvedStatement::Let {
            binding,
            mutable: true,
            ..
        } = statement
        {
            if !mutable.contains(&binding.id) {
                physical
                    .reserve(replay_btree_node_upper::<ValueId, ()>().ok_or_else(error)?)
                    .map_err(|_| error())?;
            }
            mutable.insert(binding.id.clone());
        }
        Ok(())
    })?;
    let mut legacy = BTreeMap::new();
    walk(function, &mut frames, &mut work, |statement| {
        if let ResolvedStatement::Assign {
            binding,
            field: None,
            value,
            ..
        } = statement
        {
            if mutable.contains(&binding.id)
                && crate::byte_ops::is_same_owner_set_hir(value, &binding.id)
            {
                if !legacy.contains_key(&value.id) {
                    physical
                        .reserve(
                            replay_btree_node_upper::<ExpressionId, &ResolvedBinding>()
                                .ok_or_else(error)?,
                        )
                        .map_err(|_| error())?;
                }
                legacy.insert(value.id.clone(), binding);
            }
        }
        Ok(())
    })?;
    let helpers = crate::loop_calls::byte_writer::checked_bindings_with_reservation(
        program, function, physical,
    )?;
    Ok(RenewalBindings {
        legacy,
        helpers: HelperSites::Owned(helpers),
    })
}
fn walk<'a>(
    function: &'a ResolvedFunction,
    frames: &mut [Frame<'a>; DEPTH],
    work: &mut usize,
    mut statement: impl FnMut(&'a ResolvedStatement) -> Result<(), crate::diagnostic::Diagnostic>,
) -> Result<(), crate::diagnostic::Diagnostic> {
    frames.fill(None);
    frames[0] = Some((&function.body, usize::MAX));
    let mut depth = 1;
    while depth != 0 {
        *work = work.checked_sub(1).ok_or_else(error)?;
        let (expression, next) = frames[depth - 1].ok_or_else(error)?;
        if next == usize::MAX {
            if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                *work = work.checked_sub(statements.len()).ok_or_else(error)?;
                for item in statements {
                    statement(item)?;
                }
            }
            frames[depth - 1] = Some((expression, 0));
            continue;
        }
        if matches!(expression.kind, ResolvedExprKind::Closure { .. }) {
            depth -= 1;
            continue;
        }
        let width = match &expression.kind {
            ResolvedExprKind::Block { statements, .. } => statements.len(),
            ResolvedExprKind::Match { arms, .. } => arms.len(),
            _ => 0,
        };
        *work = work.checked_sub(width).ok_or_else(error)?;
        if let Some(child) = crate::hir::lexical_exit::child(expression, next) {
            if depth == DEPTH {
                return Err(error());
            }
            frames[depth - 1] = Some((expression, next.checked_add(1).ok_or_else(error)?));
            frames[depth] = Some((child, usize::MAX));
            depth += 1;
        } else {
            depth -= 1;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn selected_primitive_renewals_match_legacy_and_reserve_before_one_short_growth() {
        let checked = crate::check(
            include_str!("../../../tests/owned_data/byte_helper_renewal.spx"),
            "selected-renewal.spx",
        )
        .unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let legacy = super::super::bindings(&program, function);
        let mut physical = ReplayPhysicalReservation::new(usize::MAX);
        let selected = bindings_with_reservation(&program, function, &mut physical).unwrap();
        let sites = crate::loop_calls::byte_writer::checked_bindings(&program, function).unwrap();
        assert_eq!(sites.len(), 2);
        for (at, _) in sites {
            assert_eq!(
                selected.get(at).map(|b| &b.id),
                legacy.get(at).map(|b| &b.id)
            );
        }
        let bytes = physical.charged();
        let mut exact = ReplayPhysicalReservation::new(bytes);
        let exact_sites = bindings_with_reservation(&program, function, &mut exact).unwrap();
        assert_eq!(
            exact_sites
                .legacy
                .iter()
                .map(|(at, b)| (at, &b.id))
                .collect::<Vec<_>>(),
            selected
                .legacy
                .iter()
                .map(|(at, b)| (at, &b.id))
                .collect::<Vec<_>>()
        );
        let leaf = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "helper.leaf")
            .unwrap();
        let leaf_legacy = super::super::bindings(&program, leaf);
        let mut leaf_physical = ReplayPhysicalReservation::new(usize::MAX);
        let leaf_selected = bindings_with_reservation(&program, leaf, &mut leaf_physical).unwrap();
        assert_eq!(leaf_selected.legacy.len(), 1);
        assert_eq!(
            leaf_selected
                .legacy
                .iter()
                .map(|(at, b)| (at, &b.id))
                .collect::<Vec<_>>(),
            leaf_legacy
                .legacy
                .iter()
                .map(|(at, b)| (at, &b.id))
                .collect::<Vec<_>>()
        );
        assert_eq!(exact.charged(), bytes);
        let mut short = ReplayPhysicalReservation::new(bytes - 1);
        assert_eq!(
            bindings_with_reservation(&program, function, &mut short)
                .err()
                .unwrap()
                .code,
            "SPX-H006"
        );
        assert!(short.reserve(0).is_err());
    }
}
