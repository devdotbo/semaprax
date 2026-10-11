//! Exact nonlexical terminal routing. Without lexical control a terminal exit
//! is selected only by authenticated plan links: a failure by its unique
//! declared status source and the exiting block's own sole `SelectFailure`;
//! success by the root region's normal scope end, which must be the plan's
//! only ordinary commit. Vectors are consumed verbatim, never sorted, merged,
//! or reinterpreted, and no certificate is inferred: live owned entry
//! parameters lack a function-entry boundary certificate and plain loops lack
//! an iteration certificate, so both refuse. This is not a general
//! nonlexical cleanup authority.
use super::*;
use crate::cleanup_plan::{CleanupBlock, CleanupTerminator, CleanupTransition, StatusSourceId};
use crate::hir::lexical_exit::ChildCursor;
use crate::hir::{ResolvedExprKind, ResolvedStatement};

/// The unique failure exit selected by `source` in a nonlexical plan.
pub(in crate::codegen) fn nonlexical_failure_exit<'p>(
    plan: &'p CleanupPlan,
    source: &StatusSourceId,
) -> Result<&'p ExitTarget, String> {
    if plan.lexical_control.is_some() {
        return Err("a lexical plan selects failures by lexical context".to_owned());
    }
    let label = || format!("failure from `{}` {:?}", source.expression, source.lane);
    if plan
        .status_sources
        .iter()
        .filter(|declared| &declared.id == source)
        .count()
        != 1
    {
        return Err(format!("{} is not exactly one declared status source", label()));
    }
    let mut matches = plan.exits.iter().filter(|exit| {
        matches!(&exit.continuation, ExitContinuation::ReturnFailure { source: selected } if selected == source)
    });
    let exit = matches
        .next()
        .ok_or_else(|| format!("{} has no failure exit", label()))?;
    if matches.next().is_some() {
        return Err(format!("{} selects more than one failure exit", label()));
    }
    let block = exiting_block(plan, exit)?;
    let mut selected = block
        .transitions
        .iter()
        .filter_map(|transition| match transition {
            CleanupTransition::SelectFailure { source } => Some(source),
            _ => None,
        });
    if selected.next() != Some(source) || selected.next().is_some() {
        return Err(format!(
            "{} is not linked to its block's sole status selection",
            exit_label(exit)
        ));
    }
    Ok(exit)
}

/// The root normal scope end, required to be the plan's only ordinary success.
pub(in crate::codegen) fn nonlexical_success_exit(
    plan: &CleanupPlan,
) -> Result<&ExitTarget, String> {
    if plan.lexical_control.is_some() {
        return Err("a lexical plan selects success by its root commit certificate".to_owned());
    }
    let root = plan
        .regions
        .first()
        .filter(|region| region.id.0 == 0 && region.parent.is_none())
        .ok_or("the root cleanup region is absent")?;
    if plan.regions[1..].iter().any(|region| region.parent.is_none()) {
        return Err("the root cleanup region is not unique".to_owned());
    }
    let exit = plan
        .exits
        .get(root.normal_scope_end.0 as usize)
        .filter(|exit| exit.id == root.normal_scope_end)
        .ok_or("the root normal scope end names no exit")?;
    if !ordinary_success(exit) {
        return Err(format!(
            "the root normal scope end {} is not an ordinary success",
            exit_label(exit)
        ));
    }
    if exit.leaves_regions.as_slice() != [root.id] {
        return Err(format!(
            "{} does not leave exactly the root region",
            exit_label(exit)
        ));
    }
    exiting_block(plan, exit)?;
    let successes = plan
        .exits
        .iter()
        .filter(|exit| ordinary_success(exit))
        .count();
    if successes != 1 {
        return Err(format!(
            "{successes} ordinary success exits exist, but only the root normal scope end may commit"
        ));
    }
    Ok(exit)
}

/// Why some ordinary terminal exit has no exact nonlexical selection.
pub(super) fn refusal(
    plan: &CleanupPlan,
    exits: &[&ExitTarget],
    by_flag: &BTreeMap<LivenessFlagId, ByteSlot>,
    plain_loop: impl FnOnce() -> bool,
) -> Option<String> {
    let entry = &plan.entry_state;
    if !entry.live_owned_parameters.is_empty() || !entry.conditional_owned_parameters.is_empty() {
        return Some(
            "owned parameters are live at entry without a function-entry boundary certificate, so call-depth or fuel refusal at entry has no exact vector"
                .to_owned(),
        );
    }
    if plain_loop() {
        return Some(
            "a plain `while` loop has no iteration execution-boundary certificate, so metered iteration fuel refusal has no exact vector"
                .to_owned(),
        );
    }
    for exit in exits {
        if let Some(action) = exit
            .finalize_in_order
            .iter()
            .find(|action| !by_flag.contains_key(&action.guard_flag))
        {
            return Some(format!(
                "{} finalizes liveness flag {} outside the physical Bytes plan",
                exit_label(exit),
                action.guard_flag.0
            ));
        }
        let selected = match &exit.continuation {
            ExitContinuation::ReturnFailure { source } => nonlexical_failure_exit(plan, source),
            ExitContinuation::CommitResult { .. } | ExitContinuation::ReturnUnit => {
                nonlexical_success_exit(plan)
            }
            _ => Err(format!(
                "{} has no exact nonlexical selection",
                exit_label(exit)
            )),
        };
        match selected {
            Ok(selected) if std::ptr::eq(selected, *exit) => {}
            Ok(_) => {
                return Some(format!(
                    "{} is not the exit its link selects",
                    exit_label(exit)
                ))
            }
            Err(reason) => return Some(reason),
        }
    }
    None
}

fn ordinary_success(exit: &ExitTarget) -> bool {
    matches!(
        exit.continuation,
        ExitContinuation::CommitResult { .. } | ExitContinuation::ReturnUnit
    )
}

/// The exit's canonical index and its block's own `Exit` terminator.
fn exiting_block<'p>(plan: &'p CleanupPlan, exit: &ExitTarget) -> Result<&'p CleanupBlock, String> {
    if !plan
        .exits
        .get(exit.id.0 as usize)
        .is_some_and(|indexed| std::ptr::eq(indexed, exit))
    {
        return Err(format!("{} is not at its canonical index", exit_label(exit)));
    }
    plan.blocks
        .get(exit.from.0 as usize)
        .filter(|block| block.id == exit.from && block.terminator == CleanupTerminator::Exit(exit.id))
        .ok_or_else(|| format!("{} does not terminate its source block", exit_label(exit)))
}

/// Any `while` in the contracts or body. Scan-depth overflow fails closed.
pub(super) fn plain_loop(function: &ResolvedFunction) -> bool {
    let mut stack = [None; crate::cache_codec::MAX_DEPTH + 1];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        stack[0] = Some(ChildCursor::new(root));
        let mut len = 1usize;
        while len != 0 {
            let frame = stack[len - 1]
                .as_mut()
                .expect("occupied loop scan frame");
            let expr = frame.expression();
            if !frame.is_started()
                && matches!(
                    &expr.kind,
                    ResolvedExprKind::Block { statements, .. }
                        | ResolvedExprKind::VecConstructor { statements, .. }
                        if statements.iter().any(|statement| matches!(
                            statement,
                            ResolvedStatement::While { .. } | ResolvedStatement::LexicalWhile { .. }
                        ))
                )
            {
                return true;
            }
            if let Some(next) = frame.next() {
                if len == stack.len() {
                    return true;
                }
                stack[len] = Some(ChildCursor::new(next));
                len += 1;
            } else {
                len -= 1;
                stack[len] = None;
            }
        }
    }
    false
}
