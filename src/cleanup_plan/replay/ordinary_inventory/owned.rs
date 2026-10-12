//! Complete saved pre-change inventory receiver.
use super::super::*;

pub(in crate::cleanup_plan::replay) fn validate_inventory_coverage(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;
    let base_len = function.cleanup.slots.len();
    if plan.slots.len() < base_len {
        return Err(replay_error(
            function,
            "cleanup plan omits storage required by the cleanup inventory",
        ));
    }

    let mut inventory_storage = BTreeMap::new();
    for inventory_slot in &function.cleanup.slots {
        let storage = inventory_storage_id(&inventory_slot.origin);
        inventory_storage.insert(inventory_slot.id, storage);
    }
    let mut independently_derived_flag = 0u32;
    for (index, inventory_slot) in function.cleanup.slots.iter().enumerate() {
        let actual = &plan.slots[index];
        let expected_storage = inventory_storage_id(&inventory_slot.origin);
        let independently_derived = expected_shape_for_type(
            program,
            function,
            &inventory_slot.ty,
            &mut independently_derived_flag,
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

    let expected_entry = function
        .cleanup
        .entry_state
        .live_owned_parameters
        .iter()
        .map(|storage| {
            inventory_storage
                .get(storage)
                .cloned()
                .map(|storage| CleanupPlace {
                    storage,
                    projections: Vec::new(),
                })
                .ok_or_else(|| {
                    replay_error(
                        function,
                        "cleanup inventory entry references unknown storage",
                    )
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    if plan.entry_state.live_owned_parameters != expected_entry {
        return Err(replay_error(
            function,
            "cleanup plan entry ownership disagrees with the independent inventory",
        ));
    }
    let expected_conditional = function
        .cleanup
        .entry_state
        .conditional_owned_parameters
        .iter()
        .map(|entry| {
            let storage = inventory_storage
                .get(&entry.storage)
                .cloned()
                .ok_or_else(|| {
                    replay_error(
                        function,
                        "conditional cleanup inventory entry references unknown storage",
                    )
                })?;
            let cases = entry
                .cases
                .iter()
                .map(|case| {
                    let live_places = case
                        .live_flags
                        .iter()
                        .map(|flag| {
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
                            Ok(CleanupPlace {
                                storage: storage.clone(),
                                projections: inventory.place.projections.clone(),
                            })
                        })
                        .collect::<Result<Vec<_>, Diagnostic>>()?;
                    Ok(ConditionalVariantCase {
                        case: case.case.clone(),
                        live_places,
                    })
                })
                .collect::<Result<Vec<_>, Diagnostic>>()?;
            Ok(ConditionalVariantEntry {
                storage,
                variant: entry.variant.clone(),
                cases,
            })
        })
        .collect::<Result<Vec<_>, Diagnostic>>()?;
    if plan.entry_state.conditional_owned_parameters != expected_conditional {
        return Err(replay_error(
            function,
            "conditional cleanup-plan entry disagrees with the independent inventory",
        ));
    }

    let mut next_flag = u32::try_from(function.cleanup.flags.len())
        .map_err(|_| replay_error(function, "too many inventory liveness flags"))?;
    let mut expected_supplemental = Vec::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        collect_supplemental_slots(
            program,
            function,
            expression,
            &mut next_flag,
            &mut expected_supplemental,
        )?;
    }
    let actual_supplemental = &plan.slots[base_len..];
    if actual_supplemental.len() != expected_supplemental.len() {
        return Err(replay_error(
            function,
            "cleanup plan has missing or extra call-argument storage",
        ));
    }
    for (offset, (actual, expected)) in actual_supplemental
        .iter()
        .zip(&expected_supplemental)
        .enumerate()
    {
        let expected_index = base_len
            .checked_add(offset)
            .ok_or_else(|| replay_error(function, "too many cleanup slots"))?;
        let expected_index = u32::try_from(expected_index)
            .map_err(|_| replay_error(function, "too many cleanup slots"))?;
        if actual.id.0 != expected_index
            || actual.storage_index != expected_index
            || actual.storage != expected.storage
            || actual.ty != expected.ty
            || actual.field_liveness_shape != expected.shape
        {
            return Err(replay_error(
                function,
                format!("supplemental call-argument slot {offset} disagrees with typed HIR"),
            ));
        }
    }
    Ok(())
}
