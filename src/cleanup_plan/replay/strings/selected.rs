//! The held current-HIR append directory lends no attached-plan authority.
use super::*;
pub(in crate::cleanup_plan::replay) fn reserve_append(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    at: &ExpressionId,
    source: &CleanupPlace,
    destination: &CleanupPlace,
    state: &mut PathState,
    budget: &mut ReplayBudget,
) -> Result<Option<Vec<LivenessFlagId>>, Diagnostic> {
    let sites = program
        .primitive_renewals(function)
        .ok_or_else(|| replay_error(function, "selected append lacks current function census"))?;
    let appends = sites.appends();
    budget.charge(
        function,
        (appends.len() + 1)
            .checked_mul(at.as_str().len() + 1)
            .and_then(|n| n.checked_mul(2))
            .ok_or_else(|| replay_error(function, "selected append lookup overflow"))?,
        "selected append exact current lookup",
    )?;
    let whole =
        |p: &CleanupPlace| matches!(p.storage, StorageId::Value(_)) && p.projections.is_empty();
    if whole(source) && appends.contains_key(at) {
        if state.renewals.contains_key(at) {
            return Err(replay_error(
                function,
                "string append reserves its owner twice",
            ));
        }
        budget.reserve_btree_node::<ExpressionId,super::super::super::renewal_history::RenewalHistory>(function,"selected append history node")?;
        budget.reserve_clone(function, &state.live_order, "selected append live history")?;
        state.renewals.insert(
            at.clone(),
            super::super::super::renewal_history::RenewalHistory::new(
                state.live_order.clone(),
                None,
            ),
        );
        return Ok(None);
    }
    if whole(destination) {
        if let Some((operand, _)) = appends.iter().find(|(_, value)| *value == at) {
            return state
                .renewals
                .remove(operand)
                .map(|h| Some(h.history))
                .ok_or_else(|| {
                    replay_error(function, "string append publishes without a reserved owner")
                });
        }
    }
    Ok(None)
}
pub(in crate::cleanup_plan::replay) fn publish_append(
    function: &ResolvedFunction,
    history: Option<Vec<LivenessFlagId>>,
    state: &mut PathState,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if let Some(history) = history {
        let capacity = history
            .len()
            .checked_add(state.live_order.len())
            .ok_or_else(|| replay_error(function, "append publication length overflow"))?;
        budget.charge(
            function,
            capacity
                .checked_mul(capacity + 1)
                .ok_or_else(|| replay_error(function, "append publication work overflow"))?,
            "selected append publication exact history",
        )?;
        budget.reserve_array::<LivenessFlagId>(
            function,
            capacity,
            "selected append publication backing",
        )?;
        let mut ordered = Vec::with_capacity(capacity);
        ordered.extend(
            history
                .iter()
                .filter(|flag| state.live_order.contains(flag))
                .copied(),
        );
        ordered.extend(
            state
                .live_order
                .iter()
                .filter(|flag| !history.contains(flag))
                .copied(),
        );
        state.live_order = ordered;
    }
    Ok(())
}
