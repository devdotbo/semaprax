//! Ordinary-context terminal Bytes cleanup. Every terminal exit's canonical
//! `finalize_in_order` vector is honored exactly; vectors are never sorted,
//! repaired, or pruned of precedence edges. One shared epilogue order exists
//! only when every exit vector is a subsequence of it. Otherwise each exit must
//! be selected at its trusted source site by lexical control, which already
//! runs that exit's exact vector, and the shared epilogue fails closed.
use super::*;
use crate::cleanup::LivenessFlagId;
use crate::cleanup_plan::{CleanupPlan, ExitTarget};

#[derive(Clone, Debug)]
pub(super) struct TerminalOrder {
    /// Shared epilogue order; empty when every exit is source-selected.
    pub(super) shared: Vec<ByteSlot>,
    /// Every slot finalized by at least one ordinary terminal exit.
    pub(super) slots: BTreeSet<ByteSlot>,
    pub(super) exit_selected: bool,
}

pub(super) fn terminal_order(
    function: &DeclarationId,
    plan: &CleanupPlan,
    by_flag: &BTreeMap<LivenessFlagId, ByteSlot>,
) -> Result<TerminalOrder, Diagnostic> {
    let exits = plan
        .exits
        .iter()
        .filter(|exit| ordinary_terminal(plan, exit))
        .collect::<Vec<_>>();
    let sequences = exits
        .iter()
        .map(|exit| {
            exit.finalize_in_order
                .iter()
                .filter_map(|action| by_flag.get(&action.guard_flag).cloned())
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let slots = sequences.iter().flatten().cloned().collect();
    if let Some(shared) = canonical_finalizer_order(&sequences)? {
        return Ok(TerminalOrder {
            shared,
            slots,
            exit_selected: false,
        });
    }
    match unselected_exit(plan, &exits) {
        None => Ok(TerminalOrder {
            shared: Vec::new(),
            slots,
            exit_selected: true,
        }),
        Some(reason) => Err(error(format!(
            "Bytes cleanup exits contain contradictory finalizer precedence in function `{function}`: {}; {reason}",
            describe_conflict(&exits, &sequences)
        ))),
    }
}

/// The exact exits that otherwise reach the common `spx_epilogue`.
fn ordinary_terminal(plan: &CleanupPlan, exit: &ExitTarget) -> bool {
    plan.lexical_block_in_context(None, exit.from)
        && !matches!(
            exit.continuation,
            ExitContinuation::Continue(_)
                | ExitContinuation::LoopLatch { .. }
                | ExitContinuation::LoopBreak { .. }
                | ExitContinuation::FunctionReturn { .. }
        )
}

/// Mirrors the native lexical router's exact ordinary-context selection: a
/// failure by its unique status source, success by the unique root commit.
/// Returns why some exit would instead reach the shared epilogue.
fn unselected_exit(plan: &CleanupPlan, exits: &[&ExitTarget]) -> Option<String> {
    let Some(control) = plan.lexical_control.as_deref() else {
        return Some(
            "this function has no lexical exit selection, so one shared native terminal epilogue cannot honor both exits"
                .to_owned(),
        );
    };
    let mut failures = BTreeMap::<_, usize>::new();
    for exit in &plan.exits {
        if let ExitContinuation::ReturnFailure { source } = &exit.continuation {
            if plan.lexical_block_in_context(None, exit.from) {
                *failures.entry(source).or_default() += 1;
            }
        }
    }
    let successes = control
        .function_success_exits
        .iter()
        .filter(|id| {
            plan.exits.get(id.0 as usize).is_some_and(|exit| {
                exit.id == **id && plan.lexical_block_in_context(None, exit.from)
            })
        })
        .count();
    for exit in exits {
        let selected = match &exit.continuation {
            ExitContinuation::ReturnFailure { source } => failures.get(source) == Some(&1),
            ExitContinuation::CommitResult { .. } | ExitContinuation::ReturnUnit => {
                successes == 1 && control.function_success_exits.contains(&exit.id)
            }
            _ => false,
        };
        if !selected {
            return Some(format!(
                "{} is not exactly selected at its source site",
                exit_label(exit)
            ));
        }
    }
    None
}

fn exit_label(exit: &ExitTarget) -> String {
    match &exit.continuation {
        ExitContinuation::ReturnFailure { source } => format!(
            "exit {} (failure from `{}` {:?})",
            exit.id.0, source.expression, source.lane
        ),
        ExitContinuation::CommitResult { .. } => format!("exit {} (result commit)", exit.id.0),
        ExitContinuation::ReturnUnit => format!("exit {} (unit return)", exit.id.0),
        _ => format!("exit {}", exit.id.0),
    }
}

/// Cold diagnostic path: the first two exits whose common slots disagree.
fn describe_conflict(exits: &[&ExitTarget], sequences: &[Vec<ByteSlot>]) -> String {
    let positions = sequences
        .iter()
        .map(|sequence| {
            sequence
                .iter()
                .enumerate()
                .map(|(index, slot)| (slot, index))
                .collect::<BTreeMap<_, _>>()
        })
        .collect::<Vec<_>>();
    for (left, sequence) in sequences.iter().enumerate() {
        for right in left + 1..sequences.len() {
            let mut previous: Option<(&ByteSlot, usize)> = None;
            for slot in sequence {
                let Some(&position) = positions[right].get(slot) else {
                    continue;
                };
                if let Some((before, at)) = previous {
                    if position < at {
                        return format!(
                            "{} finalizes `{}` at {:?} before `{}` at {:?}, but {} finalizes them in the opposite order",
                            exit_label(exits[left]),
                            before.flag,
                            before.place,
                            slot.flag,
                            slot.place,
                            exit_label(exits[right]),
                        );
                    }
                }
                previous = Some((slot, position));
            }
        }
    }
    format!(
        "{} exit finalizer orders form a precedence cycle across more than two exits",
        exits.len()
    )
}

/// Returns `None` when the exits' exact orders admit no common order.
fn canonical_finalizer_order(
    sequences: &[Vec<ByteSlot>],
) -> Result<Option<Vec<ByteSlot>>, Diagnostic> {
    let mut nodes = BTreeSet::<ByteSlot>::new();
    let mut successors = BTreeMap::<ByteSlot, BTreeSet<ByteSlot>>::new();
    let mut indegree = BTreeMap::<ByteSlot, usize>::new();
    for sequence in sequences {
        for slot in sequence {
            nodes.insert(slot.clone());
            indegree.entry(slot.clone()).or_insert(0);
        }
        for pair in sequence.windows(2) {
            if successors
                .entry(pair[0].clone())
                .or_default()
                .insert(pair[1].clone())
            {
                *indegree.entry(pair[1].clone()).or_insert(0) += 1;
            }
        }
    }
    let mut ready = indegree
        .iter()
        .filter_map(|(slot, degree)| (*degree == 0).then_some(slot.clone()))
        .collect::<BTreeSet<_>>();
    let mut order = Vec::with_capacity(nodes.len());
    while let Some(slot) = ready.pop_first() {
        order.push(slot.clone());
        for successor in successors.get(&slot).into_iter().flatten() {
            let degree = indegree
                .get_mut(successor)
                .ok_or_else(|| error("Bytes finalizer precedence node is missing"))?;
            *degree = degree
                .checked_sub(1)
                .ok_or_else(|| error("Bytes finalizer precedence underflow"))?;
            if *degree == 0 {
                ready.insert(successor.clone());
            }
        }
    }
    Ok((order.len() == nodes.len()).then_some(order))
}

#[cfg(test)]
mod tests;
