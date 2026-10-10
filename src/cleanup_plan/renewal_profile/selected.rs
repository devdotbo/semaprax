//! Metered primitive renewal reconstruction. The selected entry independently
//! closes Vec/iterator/String-replacement families before using this subset.
use super::*;
use crate::cleanup_plan::{replay_btree_node_upper, ReplayPhysicalReservation};
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedStatement, ValueId};
use std::collections::BTreeSet;
const DEPTH: usize = 514;
const WORK: usize = crate::cache_codec::MAX_NODES;
type Frame<'a> = Option<(crate::hir::lexical_exit::ChildCursor<'a>, bool)>;
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
    frames[0] = Some((
        crate::hir::lexical_exit::ChildCursor::new(&function.body),
        false,
    ));
    let mut depth = 1;
    while depth != 0 {
        *work = work.checked_sub(1).ok_or_else(error)?;
        let (cursor, entered) = frames[depth - 1].ok_or_else(error)?;
        let expression = cursor.expression();
        if !entered {
            if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                *work = work.checked_sub(statements.len()).ok_or_else(error)?;
                for item in statements {
                    statement(item)?;
                }
            }
            frames[depth - 1] = Some((cursor, true));
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
        let mut cursor = cursor;
        if let Some(child) = cursor.next() {
            if depth == DEPTH {
                return Err(error());
            }
            frames[depth - 1] = Some((cursor, true));
            frames[depth] = Some((crate::hir::lexical_exit::ChildCursor::new(child), false));
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
    #[test]
    fn cursor_walk_retains_statement_order_and_exact_legacy_work_debits() {
        let mut source = String::from("module app; @id(\"app.main\") fn main()->i64 {");
        for index in 0..32 {
            source.push_str(&format!("let value_{index}={index};"));
        }
        source.push_str("if true {let inside=4; inside} else {let later=5;later}}");
        let parsed = crate::parse(&source, "renewal-cursor-order.spx").unwrap();
        let program = crate::hir::resolve(&parsed).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let mut reference = Vec::new();
        let mut pending = vec![(&function.body, usize::MAX)];
        let mut expected_work = WORK;
        while let Some((expression, next)) = pending.pop() {
            expected_work -= 1;
            if next == usize::MAX {
                if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                    expected_work -= statements.len();
                    reference.extend(statements.iter().map(|statement| statement as *const _));
                }
                pending.push((expression, 0));
                continue;
            }
            if matches!(expression.kind, ResolvedExprKind::Closure { .. }) {
                continue;
            }
            expected_work -= match &expression.kind {
                ResolvedExprKind::Block { statements, .. } => statements.len(),
                ResolvedExprKind::Match { arms, .. } => arms.len(),
                _ => 0,
            };
            if let Some(child) = crate::hir::lexical_exit::child(expression, next) {
                pending.push((expression, next + 1));
                pending.push((child, usize::MAX));
            }
        }
        let mut frames = [None; DEPTH];
        let mut work = WORK;
        let mut actual = Vec::new();
        walk(function, &mut frames, &mut work, |statement| {
            actual.push(statement as *const _);
            Ok(())
        })
        .unwrap();
        assert_eq!(actual, reference);
        assert_eq!(work, expected_work);
        let mut short_work = WORK - expected_work - 1;
        assert_eq!(
            walk(function, &mut frames, &mut short_work, |_| Ok(()))
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
    #[test]
    fn cursor_walk_keeps_exact_fixed_depth_boundary_and_refuses_one_more() {
        let parsed = crate::parse(
            "module app; @id(\"app.main\") fn main()->i64 {0}",
            "renewal-cursor-depth.spx",
        )
        .unwrap();
        let program = crate::hir::resolve(&parsed).unwrap();
        let mut function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap()
            .clone();
        let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
            panic!("block")
        };
        let leaf = tail.as_ref().clone();
        let mut nested = leaf.clone();
        for _ in 1..DEPTH {
            let mut parent = leaf.clone();
            parent.kind = ResolvedExprKind::Block {
                statements: Vec::new(),
                tail: Box::new(nested),
            };
            nested = parent;
        }
        function.body = nested;
        let mut frames = [None; DEPTH];
        let mut work = WORK;
        walk(&function, &mut frames, &mut work, |_| Ok(())).unwrap();
        let mut parent = leaf;
        parent.kind = ResolvedExprKind::Block {
            statements: Vec::new(),
            tail: Box::new(function.body),
        };
        function.body = parent;
        assert_eq!(
            walk(&function, &mut frames, &mut work, |_| Ok(()))
                .unwrap_err()
                .code,
            "SPX-H006"
        );
    }
}
