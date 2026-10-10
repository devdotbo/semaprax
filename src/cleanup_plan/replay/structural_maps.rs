//! Selected constructor accounting for primitive structural replay indexes.
//! Independently rebuilt inventory remains the authority; attached shapes are
//! validated here and owned aggregate shapes explicitly stay unsupported.
use super::*;

pub(super) fn validate_slots(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<BTreeSet<StorageId>, Diagnostic> {
    let mut storage = BTreeSet::new();
    for (index, slot) in function.cleanup_plan.slots.iter().enumerate() {
        let expected = u32_index(function, index, "cleanup slot")?;
        if slot.id.0 != expected || slot.storage_index != expected {
            return Err(replay_error(
                function,
                format!("cleanup slot at index {index} has non-contiguous identity or index"),
            ));
        }
        if !storage.contains(&slot.storage) {
            budget.reserve_btree_node::<StorageId, ()>(function, "slot storage index")?;
            budget.reserve_clone(function, &slot.storage, "slot storage identity")?;
        }
        if !storage.insert(slot.storage.clone()) {
            return Err(replay_error(
                function,
                format!("cleanup storage `{:?}` is repeated", slot.storage),
            ));
        }
    }
    Ok(storage)
}

pub(super) fn validate_regions(
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    if plan.regions.is_empty() {
        return Err(replay_error(function, "cleanup plan has no root region"));
    }
    let mut assigned = BTreeSet::new();
    for (index, region) in plan.regions.iter().enumerate() {
        let expected = u32_index(function, index, "cleanup region")?;
        if region.id.0 != expected {
            return Err(replay_error(
                function,
                "cleanup region IDs are not contiguous",
            ));
        }
        match (index, region.parent) {
            (0, None) => {}
            (0, Some(_)) | (_, None) => {
                return Err(replay_error(
                    function,
                    "cleanup region tree has an invalid root",
                ));
            }
            (_, Some(parent)) if usize::try_from(parent.0).ok().is_some_and(|p| p < index) => {}
            _ => {
                return Err(replay_error(
                    function,
                    "cleanup region parent must precede its child",
                ));
            }
        }
        for item in &region.slots {
            if !storage.contains(item) {
                return Err(replay_error(
                    function,
                    "cleanup region references unknown storage",
                ));
            }
            if !assigned.contains(item) {
                budget.reserve_btree_node::<StorageId, ()>(function, "region storage index")?;
                budget.reserve_clone(function, item, "region storage identity")?;
            }
            if !assigned.insert(item.clone()) {
                return Err(replay_error(
                    function,
                    "cleanup storage is assigned to more than one region",
                ));
            }
        }
        let unresolved =
            plan.lexical_control.is_some() && region.normal_scope_end == UNRESOLVED_NORMAL_END;
        if !unresolved
            && usize::try_from(region.normal_scope_end.0)
                .ok()
                .is_none_or(|exit| exit >= plan.exits.len())
        {
            return Err(replay_error(
                function,
                "cleanup region references an unknown normal-scope exit",
            ));
        }
    }
    if &assigned != storage {
        return Err(replay_error(
            function,
            "cleanup plan has storage not assigned to exactly one region",
        ));
    }
    Ok(())
}

pub(super) fn collect_leaves(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Leaves, Diagnostic> {
    let mut leaves = BTreeMap::new();
    let mut places = BTreeSet::new();
    for slot in &function.cleanup_plan.slots {
        match &slot.field_liveness_shape {
            FieldLivenessShape::NoDrop => {}
            FieldLivenessShape::Leaf { flag, lifecycle } => {
                if leaves.contains_key(flag) {
                    return Err(replay_error(
                        function,
                        format!("liveness flag {} is repeated", flag.0),
                    ));
                }
                budget.reserve_btree_node::<LivenessFlagId, Leaf>(function, "leaf index")?;
                budget.reserve_clone(function, &slot.storage, "leaf storage identity")?;
                budget.reserve_clone(function, lifecycle, "leaf lifecycle identity")?;
                let place = CleanupPlace {
                    storage: slot.storage.clone(),
                    projections: Vec::new(),
                };
                let leaf = Leaf {
                    place: place.clone(),
                    lifecycle: lifecycle.clone(),
                };
                leaves.insert(*flag, leaf);
                if places.contains(&place) {
                    return Err(replay_error(function, "cleanup leaf place is repeated"));
                }
                budget.reserve_btree_node::<CleanupPlace, ()>(function, "leaf place index")?;
                places.insert(place);
            }
            FieldLivenessShape::Record { .. } | FieldLivenessShape::Variant { .. } => {
                return Err(replay_error(
                    function,
                    "selected physical leaf inventory does not support owned aggregate shapes",
                ));
            }
        }
    }
    for (expected, flag) in leaves.keys().enumerate() {
        if flag.0 != u32_index(function, expected, "liveness flag")? {
            return Err(replay_error(
                function,
                "cleanup liveness flags are not contiguous from zero",
            ));
        }
    }
    Leaves::new_with_budget(function, leaves, budget)
}

#[cfg(test)]
mod tests;
