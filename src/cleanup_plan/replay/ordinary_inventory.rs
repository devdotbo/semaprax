//! Independent current inventory comparison without cloned entry directories.
use super::*;
#[cfg(test)]
pub(super) mod owned;
pub(super) mod storage;
pub(super) mod substitute;

pub(super) fn validate_inventory_coverage(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    #[cfg(test)]
    if SAVED_RECEIVER.with(std::cell::Cell::get) {
        return owned::validate_inventory_coverage(program, function);
    }
    let plan = &function.cleanup_plan;
    let base_len = function.cleanup.slots.len();
    if plan.slots.len() < base_len {
        return Err(replay_error(
            function,
            "cleanup plan omits storage required by the cleanup inventory",
        ));
    }

    // Retain every duplicate's ordinal: the old map selected the last insertion.
    let mut inventory_storage = Vec::new();
    budget.reserve_vec(
        function,
        &mut inventory_storage,
        base_len,
        "inventory storage directory",
    )?;
    inventory_storage.extend(function.cleanup.slots.iter().enumerate());
    inventory_storage
        .sort_unstable_by(|(a, left), (b, right)| left.id.cmp(&right.id).then(a.cmp(b)));
    let storage = |id: &crate::cleanup::CleanupStorageId| {
        let end = inventory_storage.partition_point(|(_, slot)| slot.id <= *id);
        inventory_storage
            .get(end.checked_sub(1)?)
            .and_then(|(_, slot)| (slot.id == *id).then(|| inventory_storage_id(&slot.origin)))
    };
    let mut independently_derived_flag = 0u32;
    for (index, inventory_slot) in function.cleanup.slots.iter().enumerate() {
        let actual = &plan.slots[index];
        let expected_storage = inventory_storage_id(&inventory_slot.origin);
        let independently_derived = nested_shape::expected_shape_for_type_paid(
            program,
            function,
            &inventory_slot.ty,
            &mut independently_derived_flag,
            budget,
        )?;
        if inventory_slot.shape != independently_derived {
            return Err(replay_error(
                function,
                format!(
                    "cleanup inventory base slot {index} disagrees with independently derived typed HIR"
                ),
            ));
        }
        if actual.id.0 != inventory_slot.id.0
            || actual.storage_index != inventory_slot.discovery_index
            || actual.storage != expected_storage
            || actual.ty != inventory_slot.ty
            || actual.field_liveness_shape != inventory_slot.shape
        {
            return Err(replay_error(
                function,
                format!("cleanup plan base slot {index} disagrees with the independent inventory"),
            ));
        }
    }
    if usize::try_from(independently_derived_flag).ok() != Some(function.cleanup.flags.len()) {
        return Err(replay_error(
            function,
            "cleanup inventory liveness census disagrees with independently derived typed HIR",
        ));
    }

    let mut entry_equal = plan.entry_state.live_owned_parameters.len()
        == function.cleanup.entry_state.live_owned_parameters.len();
    for (ordinal, id) in function
        .cleanup
        .entry_state
        .live_owned_parameters
        .iter()
        .enumerate()
    {
        let expected = storage(id).ok_or_else(|| {
            replay_error(
                function,
                "cleanup inventory entry references unknown storage",
            )
        })?;
        entry_equal &= plan
            .entry_state
            .live_owned_parameters
            .get(ordinal)
            .is_some_and(|actual| actual.storage == expected && actual.projections.is_empty());
    }
    // Selection follows the complete old fallible collection frontier.
    if !entry_equal {
        return Err(replay_error(
            function,
            "cleanup plan entry ownership disagrees with the independent inventory",
        ));
    }
    let actual_entries = &plan.entry_state.conditional_owned_parameters;
    let expected_entries = &function.cleanup.entry_state.conditional_owned_parameters;
    let mut conditional_equal = actual_entries.len() == expected_entries.len();
    for (ordinal, entry) in expected_entries.iter().enumerate() {
        let expected_storage = storage(&entry.storage).ok_or_else(|| {
            replay_error(
                function,
                "conditional cleanup inventory entry references unknown storage",
            )
        })?;
        let actual = actual_entries.get(ordinal);
        conditional_equal &= actual.is_some_and(|actual| {
            actual.storage == expected_storage
                && actual.variant == entry.variant
                && actual.cases.len() == entry.cases.len()
        });
        for (case_ordinal, case) in entry.cases.iter().enumerate() {
            let actual_case = actual.and_then(|actual| actual.cases.get(case_ordinal));
            conditional_equal &= actual_case.is_some_and(|actual| {
                actual.case == case.case && actual.live_places.len() == case.live_flags.len()
            });
            for (place_ordinal, flag) in case.live_flags.iter().enumerate() {
                // The old receiver deliberately selected the first duplicate flag.
                let inventory = function
                    .cleanup
                    .flags
                    .iter()
                    .find(|candidate| candidate.id == *flag)
                    .ok_or_else(|| {
                        replay_error(
                            function,
                            "conditional cleanup entry references unknown flag",
                        )
                    })?;
                if inventory.place.storage != entry.storage
                    || inventory.place.projections.first() != Some(&case.case)
                {
                    return Err(replay_error(
                        function,
                        "conditional cleanup entry flag has a foreign case path",
                    ));
                }
                conditional_equal &= actual_case
                    .and_then(|case| case.live_places.get(place_ordinal))
                    .is_some_and(|actual| {
                        actual.storage == expected_storage
                            && actual.projections == inventory.place.projections
                    });
            }
        }
    }
    if !conditional_equal {
        return Err(replay_error(
            function,
            "conditional cleanup-plan entry disagrees with the independent inventory",
        ));
    }

    let mut next_flag = u32::try_from(function.cleanup.flags.len())
        .map_err(|_| replay_error(function, "too many inventory liveness flags"))?;
    let actual_supplemental = &plan.slots[base_len..];
    let mut scratch = supplemental::receiving::Scratch::new(function, budget)?;
    let mut count = 0usize;
    let mut first_mismatch = None;
    let mut first_index_overflow = None;
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        supplemental::receiving::walk(
            program,
            function,
            expression,
            &mut next_flag,
            &mut scratch,
            budget,
            |storage, ty, shape| {
                let offset = count;
                count = count
                    .checked_add(1)
                    .ok_or_else(|| replay_error(function, "too many cleanup slots"))?;
                match base_len
                    .checked_add(offset)
                    .and_then(|index| u32::try_from(index).ok())
                {
                    Some(index) => {
                        if !actual_supplemental.get(offset).is_some_and(|actual| {
                            actual.id.0 == index
                                && actual.storage_index == index
                                && actual.storage == storage
                                && actual.ty == *ty
                                && actual.field_liveness_shape == shape
                        }) && first_mismatch.is_none()
                        {
                            first_mismatch = Some(offset);
                        }
                    }
                    None if first_index_overflow.is_none() => first_index_overflow = Some(offset),
                    None => {}
                }
                Ok(())
            },
        )?;
    }
    if actual_supplemental.len() != count {
        return Err(replay_error(
            function,
            "cleanup plan has missing or extra call-argument storage",
        ));
    }
    if first_index_overflow
        .is_some_and(|offset| first_mismatch.is_none_or(|mismatch| offset <= mismatch))
    {
        return Err(replay_error(function, "too many cleanup slots"));
    }
    if let Some(offset) = first_mismatch {
        return Err(replay_error(
            function,
            format!("supplemental call-argument slot {offset} disagrees with typed HIR"),
        ));
    }

    Ok(())
}

#[cfg(test)]
thread_local! {
    static SAVED_RECEIVER: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
fn with_saved_receiver<T>(operation: impl FnOnce() -> T) -> T {
    struct Reset(bool);
    impl Drop for Reset {
        fn drop(&mut self) {
            SAVED_RECEIVER.with(|cell| cell.set(self.0));
        }
    }
    let _reset = Reset(SAVED_RECEIVER.with(|cell| cell.replace(true)));
    operation()
}
#[cfg(test)]
mod tests;
