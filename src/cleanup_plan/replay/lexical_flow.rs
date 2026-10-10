//! Exact ownership-state obligations at lexical loop entries, latches and breaks.
//!
//! The proof CFG is acyclic: a loop is lowered once and its iteration end is a
//! terminal latch exit. Replay therefore proves the loop invariant directly,
//! by requiring every latch to restore the exact private ownership state that
//! flowed along the loop's true-condition edge, including live order,
//! conditional variant cases and reserved or staged renewal epochs.
use super::lexical_cert::LexicalAuth;
use super::*;
use crate::hir::{ControlTarget, LoopLatchKind};

fn loop_index(auth: &LexicalAuth, target: &ControlTarget) -> Option<usize> {
    match target {
        ControlTarget::Loop { site, .. } => auth.loops.get(site).copied(),
        ControlTarget::Function(_) => None,
    }
}

fn restores(entry: &PathState, state: &PathState) -> bool {
    entry.live_order == state.live_order
        && entry.conditional_variants == state.conditional_variants
        && entry.renewals == state.renewals
}

/// Record the state a loop body starts from when its branch is replayed.
pub(super) fn capture_loop_entry(
    function: &ResolvedFunction,
    auth: &LexicalAuth,
    condition: &EdgeCondition,
    state: &PathState,
    entries: &mut BTreeMap<usize, PathState>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let EdgeCondition::BooleanResult(expression, true) = condition else {
        return Ok(());
    };
    let Some(index) = auth.loops.get(expression).copied() else {
        return Ok(());
    };
    let certificate = function
        .cleanup_plan
        .lexical_control
        .as_deref()
        .and_then(|control| control.loops.get(index))
        .ok_or_else(|| replay_error(function, "loop entry has no certificate"))?;
    if certificate.entry_order != state.live_order {
        return Err(replay_error(
            function,
            "loop certificate entry order differs from the replayed entry ownership",
        ));
    }
    if let Some(previous) = entries.get(&index) {
        return if previous == state { Ok(()) } else { Err(replay_error(
            function, "loop is entered with two different ownership states")) };
    }
    budget.reserve_btree_node::<usize, PathState>(function, "lexical loop snapshot node")?;
    path_join::storage::reserve_state_clone(function, state, budget, "lexical loop snapshot")?;
    entries.insert(index, state.clone());
    Ok(())
}

/// A loop latch ends one iteration: it must restore the exact entry state.
pub(super) fn check_latch(
    function: &ResolvedFunction,
    auth: &LexicalAuth,
    exit: &ExitTarget,
    state: &PathState,
    entries: &BTreeMap<usize, PathState>,
) -> Result<(), Diagnostic> {
    let ExitContinuation::LoopLatch { target } = &exit.continuation else {
        return Err(replay_error(function, "latch check on a non-latch exit"));
    };
    let entry = loop_index(auth, target)
        .and_then(|index| entries.get(&index))
        .ok_or_else(|| replay_error(function, "loop latch precedes its loop entry state"))?;
    if restores(entry, state) {
        Ok(())
    } else {
        Err(replay_error(
            function,
            "loop iteration does not restore its exact entry ownership state",
        ))
    }
}

/// A `break` of a counter or Copy loop joins the loop continuation, so its state
/// must equal the entry state exactly. An owning traversal enters its wrapper
/// fragment instead: the consumed hidden Step must be dead here, and the exact
/// parent join is checked where the fragment rejoins ordinary flow.
pub(super) fn check_break(
    function: &ResolvedFunction,
    auth: &LexicalAuth,
    exit: &ExitTarget,
    state: &PathState,
    entries: &BTreeMap<usize, PathState>,
    leaves: &Leaves,
) -> Result<(), Diagnostic> {
    let ExitContinuation::LoopBreak { target, .. } = &exit.continuation else {
        return Ok(());
    };
    let index = loop_index(auth, target)
        .ok_or_else(|| replay_error(function, "loop break names an unknown loop"))?;
    let certificate = function
        .cleanup_plan
        .lexical_control
        .as_deref()
        .and_then(|control| control.loops.get(index))
        .ok_or_else(|| replay_error(function, "loop break has no certificate"))?;
    if let LoopLatchKind::OwnedIteratorStep { step, .. } = &certificate.latch {
        // The hidden Step was consumed entering Yield; it cannot reappear on a
        // break path merely to agree with the loop's false-condition branch.
        let step = StorageId::Value(step.clone());
        let live = state
            .live_order
            .iter()
            .any(|flag| leaves[flag].place.storage == step)
            || state
                .conditional_variants
                .iter()
                .any(|variant| variant.root.storage == step);
        if live {
            return Err(replay_error(
                function,
                "owning break retains a live hidden Step",
            ));
        }
        return Ok(());
    }
    let entry = entries
        .get(&index)
        .ok_or_else(|| replay_error(function, "loop break precedes its loop entry state"))?;
    if restores(entry, state) {
        Ok(())
    } else {
        Err(replay_error(
            function,
            "loop break does not restore its exact surrounding ownership state",
        ))
    }
}
