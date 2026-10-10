//! Typed-skeleton markers for lexical exits and loop latches.
//!
//! A lexical exit is a terminal flow inside its expression. The typed
//! skeleton records it as one marker observation and then reuses the existing
//! short-circuit state (`failed`), so no ordinary statement, call or arm
//! continues after it. The plan-side walk emits the very same marker from the
//! exit certificate, which is how the comparison authenticates exit order,
//! target and kind without trusting either side:
//!
//! * `return` leaves its contour, then continues through every postcondition;
//! * `break` of the nearest loop continues after that loop;
//! * `continue` runs the loop's generated advancement and ends at the latch;
//! * a normally completed iteration ends at the latch.
use super::*;
use crate::ast::LexicalExitKind;
use crate::cleanup_plan::ExitTargetId;
use crate::hir::ControlTarget;

/// Plan-side marker of a certified lexical exit; `None` for an uncertified
/// loop latch (normal iteration end or generated-advancement end).
pub(super) fn plan_exit_observation(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    exit: ExitTargetId,
) -> Result<Option<SkeletonObservation>, Diagnostic> {
    let Some(control) = function.cleanup_plan.lexical_control.as_deref() else {
        return Ok(None);
    };
    let Some(certificate) = control
        .exits
        .iter()
        .find(|certificate| certificate.exit == exit)
    else {
        return Ok(None);
    };
    Ok(Some(SkeletonObservation::LexicalExit {
        site: skeleton_clone(budget, function, &certificate.site, "plan exit site clone")?,
        kind: certificate.kind,
        target: skeleton_clone(
            budget,
            function,
            &certificate.target,
            "plan exit target clone",
        )?,
    }))
}

pub(super) enum Marker<'a> {
    Exit {
        kind: LexicalExitKind,
        target: &'a ControlTarget,
    },
    Latch,
}

/// The lexical marker a terminal path ended with, if any.
pub(super) fn marker(path: &ExprSkeletonPath) -> Option<Marker<'_>> {
    match &**path.observations.last()? {
        SkeletonObservation::LexicalExit { kind, target, .. } => Some(Marker::Exit {
            kind: *kind,
            target,
        }),
        SkeletonObservation::LoopLatch { .. } => Some(Marker::Latch),
        _ => None,
    }
}

/// How one completed typed-HIR path ends.
pub(super) fn terminal(path: &ExprSkeletonPath) -> SkeletonTerminal {
    if !path.failed {
        return SkeletonTerminal::Success;
    }
    match marker(path) {
        Some(Marker::Latch)
        | Some(Marker::Exit {
            kind: LexicalExitKind::Continue,
            ..
        }) => SkeletonTerminal::LoopLatch,
        _ => SkeletonTerminal::Failure,
    }
}

fn exit_marker(
    expression: &ResolvedExpr,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<SkeletonObservation, Diagnostic> {
    let ResolvedExprKind::LexicalExit { kind, target, .. } = &expression.kind else {
        return Err(replay_error(
            work.function,
            "lexical exit marker requested for an ordinary expression",
        ));
    };
    Ok(SkeletonObservation::LexicalExit {
        site: work.clone_owned(&expression.id, "lexical exit site clone")?,
        kind: *kind,
        target: work.clone_owned(target, "lexical exit target clone")?,
    })
}

/// One terminal path for an operand-free `break` or latch-free `continue`.
pub(super) fn exit_paths(
    expression: &ResolvedExpr,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut path = empty_expr_path();
    let observation = exit_marker(expression, work)?;
    work.push_observation(&mut path, observation, "lexical exit observation")?;
    path.failed = true;
    work.singleton_path(path, "lexical exit path")
}

/// The not-yet-terminal marker prefix of a `continue` that must still run its
/// generated advancement before reaching the latch.
pub(super) fn continue_prefix(
    expression: &ResolvedExpr,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut path = empty_expr_path();
    let observation = exit_marker(expression, work)?;
    work.push_observation(&mut path, observation, "continue exit observation")?;
    work.singleton_path(path, "continue prefix path")
}

fn push_latch(
    path: &mut ExprSkeletonPath,
    target: &ControlTarget,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<(), Diagnostic> {
    let target = work.clone_owned(target, "loop latch target clone")?;
    work.push_observation(
        path,
        SkeletonObservation::LoopLatch { target },
        "loop latch observation",
    )?;
    path.failed = true;
    path.owned_source = None;
    Ok(())
}

/// A `return` operand finished: its paths leave their contour. A non-failed
/// path becomes terminal until the function epilogue releases it.
pub(super) fn mark_return(
    paths: Vec<ExprSkeletonPath>,
    expression: &ResolvedExpr,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut marked = Vec::new();
    for mut path in paths {
        if !path.failed && !path.residual {
            let observation = exit_marker(expression, work)?;
            work.push_observation(&mut path, observation, "lexical return observation")?;
            path.failed = true;
            path.owned_source = None;
        }
        work.push_expr_path(&mut marked, path, "lexical return path")?;
    }
    Ok(marked)
}

/// A `continue` advancement finished: its normal paths reach the latch.
pub(super) fn finish_continue(
    paths: Vec<ExprSkeletonPath>,
    target: &ControlTarget,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut finished = Vec::new();
    for mut path in paths {
        if !path.failed && !path.residual {
            push_latch(&mut path, target, work)?;
        }
        work.push_expr_path(&mut finished, path, "continue latch path")?;
    }
    Ok(finished)
}

/// Settle one loop's body paths after the true-condition prefixes are joined:
/// a normal iteration ends at the latch, a `break` of this loop continues
/// after it, and everything else (failure, `return`, `continue`, an outer
/// loop's exit) stays terminal.
pub(super) fn settle_loop_body(
    joined: Vec<ExprSkeletonPath>,
    target: &ControlTarget,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let mut settled = Vec::new();
    for mut path in joined {
        // A residual `?` path is already terminal for this loop and passes through.
        if !path.failed && !path.residual {
            push_latch(&mut path, target, work)?;
        } else if matches!(
            marker(&path),
            Some(Marker::Exit { kind: LexicalExitKind::Break, target: exit }) if exit == target
        ) {
            path.failed = false;
        }
        work.push_expr_path(&mut settled, path, "lexical loop body path")?;
    }
    Ok(settled)
}

/// Release every `return` path into the postcondition sequence.
pub(super) fn release_returns(paths: &mut [ExprSkeletonPath]) {
    for path in paths {
        if path.failed
            && matches!(
                marker(path),
                Some(Marker::Exit {
                    kind: LexicalExitKind::Return,
                    ..
                })
            )
        {
            path.failed = false;
        }
    }
}
