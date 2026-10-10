use super::*;
pub(in crate::cleanup_plan::replay) fn validate_entry(
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if function
        .cleanup_plan
        .blocks
        .get(function.cleanup_plan.entry.0 as usize)
        .is_none()
    {
        return Err(replay_error(function, "cleanup entry block is unknown"));
    }
    let mut flags = BTreeSet::new();
    for place in &function.cleanup_plan.entry_state.live_owned_parameters {
        for flag in path_join::storage::validate_place(function, place, storage, leaves, budget)? {
            if !insert_copy(function, budget, &mut flags, flag, "structural entry flags")? {
                return Err(replay_error(
                    function,
                    "cleanup entry places overlap or repeat liveness flags",
                ));
            }
        }
    }
    Ok(())
}
