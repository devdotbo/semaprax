//! Saved dd705 ordinary storage/leaf algorithms, before payer changes.
use super::*;
pub(super) fn validate_slots(
    function: &ResolvedFunction,
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
        if !storage.insert(slot.storage.clone()) {
            return Err(replay_error(
                function,
                format!("cleanup storage `{:?}` is repeated", slot.storage),
            ));
        }
    }
    Ok(storage)
}

pub(super) fn collect_leaves(function: &ResolvedFunction) -> Result<Leaves, Diagnostic> {
    let mut leaves = BTreeMap::new();
    let mut places = BTreeSet::new();
    for slot in &function.cleanup_plan.slots {
        collect_shape(
            function,
            &slot.storage,
            &mut Vec::new(),
            &slot.field_liveness_shape,
            &mut leaves,
            &mut places,
        )?;
    }
    for (expected, flag) in leaves.keys().enumerate() {
        if flag.0 != u32_index(function, expected, "liveness flag")? {
            return Err(replay_error(
                function,
                "cleanup liveness flags are not contiguous from zero",
            ));
        }
    }
    Ok(Leaves::new(leaves))
}

fn collect_shape(
    function: &ResolvedFunction,
    storage: &StorageId,
    projections: &mut Vec<DeclarationId>,
    shape: &FieldLivenessShape,
    leaves: &mut BTreeMap<LivenessFlagId, Leaf>,
    places: &mut BTreeSet<CleanupPlace>,
) -> Result<(), Diagnostic> {
    enum Item<'a> {
        Shape(&'a FieldLivenessShape),
        Push(&'a DeclarationId),
        Pop,
    }

    let initial_projection_depth = projections.len();
    let mut pending = vec![Item::Shape(shape)];
    let mut work = 0usize;
    while let Some(item) = pending.pop() {
        work = work
            .checked_add(1)
            .ok_or_else(|| replay_error(function, "cleanup shape replay work overflowed"))?;
        if work > MAX_REPLAY_WORK_UNITS {
            return Err(replay_error(
                function,
                "cleanup shape replay exceeds the global work budget",
            ));
        }
        match item {
            Item::Push(projection) => {
                projections.try_reserve(1).map_err(|_| {
                    replay_error(
                        function,
                        "cleanup projection capacity exceeds address space",
                    )
                })?;
                projections.push(projection.clone());
            }
            Item::Pop => {
                if projections.len() == initial_projection_depth {
                    return Err(replay_error(
                        function,
                        "cleanup projection stack is unbalanced",
                    ));
                }
                projections.pop();
            }
            Item::Shape(FieldLivenessShape::NoDrop) => {}
            Item::Shape(FieldLivenessShape::Leaf { flag, lifecycle }) => {
                let leaf = Leaf {
                    place: CleanupPlace {
                        storage: storage.clone(),
                        projections: projections.clone(),
                    },
                    lifecycle: lifecycle.clone(),
                };
                if leaves.insert(*flag, leaf.clone()).is_some() {
                    return Err(replay_error(
                        function,
                        format!("liveness flag {} is repeated", flag.0),
                    ));
                }
                if !places.insert(leaf.place) {
                    return Err(replay_error(function, "cleanup leaf place is repeated"));
                }
            }
            Item::Shape(FieldLivenessShape::Record { fields, .. }) => {
                let mut field_ids = BTreeSet::new();
                for (index, field) in fields.iter().enumerate() {
                    let expected = u32_index(function, index, "cleanup field")?;
                    if field.field_index != expected || !field_ids.insert(field.field.clone()) {
                        return Err(replay_error(
                            function,
                            "record cleanup shape has non-contiguous or repeated fields",
                        ));
                    }
                }
                pending
                    .try_reserve(fields.len().saturating_mul(3))
                    .map_err(|_| {
                        replay_error(
                            function,
                            "cleanup shape replay capacity exceeds address space",
                        )
                    })?;
                for field in fields.iter().rev() {
                    pending.push(Item::Pop);
                    pending.push(Item::Shape(&field.shape));
                    pending.push(Item::Push(&field.field));
                }
            }
            Item::Shape(FieldLivenessShape::Variant { declaration, cases }) => {
                if !cases.is_empty()
                    && function
                        .cleanup_plan
                        .slots
                        .iter()
                        .find(|slot| slot.storage == *storage)
                        .and_then(|slot| match &slot.ty {
                            ResolvedType::Nominal { declaration, .. } => Some(declaration),
                            _ => None,
                        })
                        != Some(declaration)
                {
                    return Err(replay_error(
                        function,
                        "variant cleanup shape declaration disagrees with storage type",
                    ));
                }
                let mut case_ids = BTreeSet::new();
                let mut item_count = 0usize;
                for (case_index, case) in cases.iter().enumerate() {
                    if case.case_index != u32_index(function, case_index, "cleanup variant case")?
                        || !case_ids.insert(case.case.clone())
                    {
                        return Err(replay_error(
                            function,
                            "variant cleanup shape has non-contiguous or repeated cases",
                        ));
                    }
                    let mut field_ids = BTreeSet::new();
                    for (field_index, field) in case.fields.iter().enumerate() {
                        if field.field_index
                            != u32_index(function, field_index, "cleanup variant field")?
                            || !field_ids.insert(field.field.clone())
                        {
                            return Err(replay_error(
                                function,
                                "variant cleanup case has non-contiguous or repeated fields",
                            ));
                        }
                    }
                    item_count = item_count
                        .checked_add(2)
                        .and_then(|count| {
                            case.fields
                                .len()
                                .checked_mul(3)
                                .and_then(|fields| count.checked_add(fields))
                        })
                        .ok_or_else(|| {
                            replay_error(function, "cleanup shape replay work overflowed")
                        })?;
                }
                pending.try_reserve(item_count).map_err(|_| {
                    replay_error(
                        function,
                        "cleanup shape replay capacity exceeds address space",
                    )
                })?;
                for case in cases.iter().rev() {
                    pending.push(Item::Pop);
                    for field in case.fields.iter().rev() {
                        pending.push(Item::Pop);
                        pending.push(Item::Shape(&field.shape));
                        pending.push(Item::Push(&field.field));
                    }
                    pending.push(Item::Push(&case.case));
                }
            }
        }
    }
    if projections.len() != initial_projection_depth {
        return Err(replay_error(
            function,
            "cleanup projection stack remains unbalanced",
        ));
    }
    Ok(())
}
