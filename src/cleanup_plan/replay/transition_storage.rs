//! Selected transfer scratch is reserved before construction. Conditional case
//! flags can be remapped in place; canonical initialization order stays intact.
use super::*;

#[allow(clippy::too_many_arguments)]
pub(super) fn execute(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    transition: &CleanupTransition,
    state: &mut PathState,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if state.published {
        return Err(replay_error(
            function,
            "cleanup transition occurs after result publication",
        ));
    }
    if state.selected_failure.is_some()
        && !matches!(transition, CleanupTransition::SelectFailure { .. })
    {
        return Err(replay_error(
            function,
            "ordinary cleanup transition occurs after failure selection",
        ));
    }
    if state.pending_failure.is_some()
        && !matches!(transition, CleanupTransition::SelectFailure { .. })
    {
        return Err(replay_error(
            function,
            "operation failure is not selected before another transition",
        ));
    }
    if !state.conditional_variants.is_empty() {
        return Err(replay_error(
            function,
            "selected primitive replay has conditional aggregate state",
        ));
    }
    match transition {
        CleanupTransition::Initialize { destination, .. } => {
            let flags =
                path_join::storage::validate_place(function, destination, storage, leaves, budget)?;
            // append_dead_flags performs the same validity check before growth.
            if flags.is_empty() || flags.iter().any(|flag| state.live_order.contains(flag)) {
                return Err(replay_error(
                    function,
                    "initialize transition initializes an empty or live cleanup place",
                ));
            }
            budget.reserve_vec(
                function,
                &mut state.live_order,
                flags.len(),
                "initialized live history",
            )?;
            append_dead_flags(function, state, flags, "initialize transition")?;
        }
        CleanupTransition::ReserveRenewal { at, binding } => {
            renewal::reserve(
                program, function, at, binding, state, storage, leaves, helpers, budget,
            )?;
        }
        CleanupTransition::Renew {
            at,
            source,
            destination,
        } => {
            renewal::renew(
                program,
                function,
                at,
                (source, destination),
                state,
                storage,
                leaves,
                helpers,
                budget,
            )?;
        }
        CleanupTransition::Transfer {
            at,
            source,
            destination,
        } => {
            renewal::reject_unmarked_finish(program, function, at, destination, helpers)?;
            // The selected entry independently excludes concat reopen and Try
            // routes, whose delegated scratch is outside this primitive tranche.
            transfer(
                function,
                state,
                source,
                destination,
                storage,
                leaves,
                budget,
            )?;
        }
        CleanupTransition::CallCommit { call, arguments } => {
            let mut consumed = BTreeSet::new();
            for argument in arguments {
                for flag in path_join::storage::validate_place(
                    function,
                    &argument.source,
                    storage,
                    leaves,
                    budget,
                )? {
                    if !state.live_order.contains(&flag) {
                        return Err(replay_error(
                            function,
                            format!(
                                "call `{call}` atomically consumes dead argument flag {}",
                                flag.0
                            ),
                        ));
                    }
                    if consumed.contains(&flag) {
                        return Err(replay_error(
                            function,
                            format!(
                                "call `{call}` atomically consumes argument flag {} twice",
                                flag.0
                            ),
                        ));
                    }
                    budget.reserve_btree_node::<LivenessFlagId, ()>(
                        function,
                        "committed argument flags",
                    )?;
                    consumed.insert(flag);
                }
            }
            for history in state.renewals.values_mut() {
                history.commit(&consumed);
            }
            state.live_order.retain(|flag| !consumed.contains(flag));
        }
        CleanupTransition::SelectFailure { source } => {
            state.renewals.clear();
            if state.selected_failure.is_some() {
                return Err(replay_error(
                    function,
                    "failure selection is not write-once during path replay",
                ));
            }
            if state.pending_failure.as_ref() != Some(source) {
                return Err(replay_error(
                    function,
                    format!(
                        "selected failure `{}` does not match the pending failing edge",
                        source.expression
                    ),
                ));
            }
            state.pending_failure = None;
            state.selected_failure = Some(source.clone());
        }
        CleanupTransition::StageCopyResult { source } => {
            if state.staged_copy_result.is_some() {
                return Err(replay_error(
                    function,
                    "Copy aggregate result is staged more than once on one path",
                ));
            }
            if !state.live_order.is_empty() {
                return Err(replay_error(
                    function,
                    "Copy aggregate result staging carries resource liveness",
                ));
            }
            budget.reserve_clone(function, source, "staged Copy producer")?;
            state.staged_copy_result = Some(source.clone());
        }
        CleanupTransition::InitializeVariant { .. }
        | CleanupTransition::TransferVariant { .. }
        | CleanupTransition::AuthenticateVariantCase { .. } => {
            return Err(replay_error(
                function,
                "selected primitive replay excludes conditional aggregate transitions",
            ));
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub(super) fn transfer(
    function: &ResolvedFunction,
    state: &mut PathState,
    source: &CleanupPlace,
    destination: &CleanupPlace,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let source_flags =
        path_join::storage::validate_place(function, source, storage, leaves, budget)?;
    let destination_flags =
        path_join::storage::validate_place(function, destination, storage, leaves, budget)?;
    if source_flags.len() != destination_flags.len()
        || source_flags
            .iter()
            .any(|flag| destination_flags.contains(flag))
    {
        return Err(replay_error(
            function,
            "cleanup transfer does not have disjoint equal-size ownership epochs",
        ));
    }
    let conditional = state
        .conditional_variants
        .iter()
        .position(|variant| variant.root == *source);
    if conditional.is_none() {
        if source_flags
            .iter()
            .any(|flag| !state.live_order.contains(flag))
        {
            return Err(replay_error(
                function,
                "cleanup transfer reads a dead source leaf",
            ));
        }
        if destination_flags
            .iter()
            .any(|flag| state.live_order.contains(flag))
        {
            return Err(replay_error(
                function,
                "cleanup transfer initializes a live destination leaf",
            ));
        }
    }
    budget.reserve_array::<(LivenessFlagId, LivenessFlagId)>(
        function,
        source_flags.len(),
        "ownership transfer mapping",
    )?;
    let mut mapping = Vec::with_capacity(source_flags.len());
    for source_flag in &source_flags {
        let source_leaf = &leaves[source_flag];
        let relative = source_leaf
            .place
            .projections
            .strip_prefix(source.projections.as_slice())
            .ok_or_else(|| replay_error(function, "transfer source is not a projection prefix"))?;
        let mut matched = None;
        for (candidate, leaf) in leaves.iter() {
            budget.charge(function, 1, "selected transfer leaf correspondence")?;
            if leaf.place.storage == destination.storage
                && leaf.place.projections.len() == destination.projections.len() + relative.len()
                && leaf.place.projections.starts_with(&destination.projections)
                && leaf.place.projections[destination.projections.len()..] == *relative
            {
                matched = Some((*candidate, leaf));
                break;
            }
        }
        let Some((destination_flag, destination_leaf)) = matched else {
            return Err(replay_error(
                function,
                "cleanup transfer leaf shapes do not correspond",
            ));
        };
        if source_leaf.lifecycle != destination_leaf.lifecycle
            || mapping.iter().any(|(_, flag)| *flag == destination_flag)
        {
            return Err(replay_error(
                function,
                "cleanup transfer changes a leaf lifecycle or aliases its destination",
            ));
        }
        mapping.push((*source_flag, destination_flag));
    }
    if mapping.is_empty() {
        return Err(replay_error(
            function,
            "cleanup transfer has no mapped ownership leaves",
        ));
    }
    if let Some(index) = conditional {
        // Validate all flags before mutating this path. No rebuilding or sorting
        // the case vectors is needed to preserve their existing canonical order.
        if state.conditional_variants[index]
            .cases
            .iter()
            .any(|(_, flags)| {
                flags
                    .iter()
                    .any(|flag| !mapping.iter().any(|(source, _)| source == flag))
            })
        {
            return Err(replay_error(
                function,
                "conditional cleanup transfer omits a case leaf",
            ));
        }
        budget.reserve_clone(function, destination, "conditional destination place")?;
        let variant = &mut state.conditional_variants[index];
        for (_, flags) in &mut variant.cases {
            for flag in flags {
                *flag = mapping
                    .iter()
                    .find(|(source, _)| source == flag)
                    .expect("validated complete conditional transfer mapping")
                    .1;
            }
        }
        variant.root = destination.clone();
        return Ok(());
    }
    // Preserve partial-field history before removing source flags. Whole-source
    // transfers use structural source order, as in ordinary replay.
    let history_count = if source.projections.is_empty() {
        source_flags.len()
    } else {
        state
            .live_order
            .iter()
            .filter(|flag| source_flags.contains(flag))
            .count()
    };
    budget.reserve_array::<LivenessFlagId>(function, history_count, "partial transfer history")?;
    let source_history = if source.projections.is_empty() {
        source_flags.clone()
    } else {
        let mut history = Vec::with_capacity(history_count);
        history.extend(
            state
                .live_order
                .iter()
                .filter(|flag| source_flags.contains(flag))
                .copied(),
        );
        history
    };
    let after_remove = state
        .live_order
        .len()
        .checked_sub(source_flags.len())
        .ok_or_else(|| replay_error(function, "cleanup transfer source history is incomplete"))?;
    let add = if destination.projections.is_empty() {
        destination_flags.len()
    } else {
        source_history.len()
    };
    let required = after_remove
        .checked_add(add)
        .ok_or_else(|| replay_error(function, "cleanup transfer history length overflowed"))?;
    // Reserve while the old live buffer and mapping/history scratch remain held.
    if required > state.live_order.capacity() {
        let extra = required.saturating_sub(state.live_order.len());
        budget.reserve_vec(
            function,
            &mut state.live_order,
            extra,
            "transferred live history",
        )?;
    }
    for history in state.renewals.values_mut() {
        history.stage(|flag| {
            mapping
                .iter()
                .find(|(source, _)| *source == flag)
                .map(|(_, target)| *target)
        });
    }
    state.live_order.retain(|flag| !source_flags.contains(flag));
    if destination.projections.is_empty() {
        state.live_order.extend(destination_flags);
    } else {
        state
            .live_order
            .extend(source_history.into_iter().map(|flag| {
                mapping
                    .iter()
                    .find(|(source, _)| *source == flag)
                    .expect("validated source transfer mapping")
                    .1
            }));
    }
    Ok(())
}

/// Compare finalizers directly with borrowed live history. No ownership set,
/// conditional guard copies, or replacement cleanup order is materialized.
#[allow(clippy::too_many_arguments)]
pub(super) fn exit(
    function: &ResolvedFunction,
    exit: &ExitTarget,
    mut state: PathState,
    storage_regions: &BTreeMap<StorageId, CleanupRegionId>,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    extra_disposed: &BTreeSet<StorageId>,
    budget: &mut ReplayBudget,
) -> Result<ExitFlow, Diagnostic> {
    if state.published {
        return Err(replay_error(function, "cleanup exit follows publication"));
    }
    if !state.conditional_variants.is_empty() {
        return Err(replay_error(
            function,
            "selected primitive exit has conditional ownership",
        ));
    }
    let protected = match &exit.continuation {
        ExitContinuation::CommitResult {
            source: CleanupResultSource::Owned { storage },
        } => Some(storage),
        _ => None,
    };
    if let Some(result) = protected {
        // Preserve ordinary unknown/empty-place refusal before finalizer checks.
        path_join::storage::validate_place(function, result, storage, leaves, budget)?;
    }
    let protects = |flag: &LivenessFlagId| {
        protected.is_some_and(|place| {
            let leaf = &leaves[flag];
            leaf.place.storage == place.storage
                && leaf.place.projections.starts_with(&place.projections)
        })
    };
    let leaves_region = |flag: &LivenessFlagId| {
        let leaf = &leaves[flag];
        exit.leaves_regions
            .contains(&storage_regions[&leaf.place.storage])
            || extra_disposed.contains(&leaf.place.storage)
    };
    let expected = state
        .live_order
        .iter()
        .rev()
        .filter(|flag| leaves_region(flag) && !protects(flag));
    let actual = exit.finalize_in_order.iter().filter_map(|action| {
        state
            .live_order
            .contains(&action.guard_flag)
            .then_some(&action.guard_flag)
    });
    if expected.ne(actual) {
        return Err(replay_error(
            function,
            format!(
                "exit {} does not finalize its live region leaves in reverse initialization order",
                exit.id.0
            ),
        ));
    }
    if exit
        .finalize_in_order
        .iter()
        .any(|action| action.active_case.is_some())
    {
        return Err(replay_error(
            function,
            "exit does not preserve exact conditional variant finalizers",
        ));
    }
    state
        .live_order
        .retain(|flag| !(leaves_region(flag) && !protects(flag)));
    if state.live_order.iter().any(|flag| {
        exit.leaves_regions
            .contains(&storage_regions[&leaves[flag].place.storage])
            && !protects(flag)
    }) {
        return Err(replay_error(
            function,
            "cleanup exit retains a live non-result leaf in a region it leaves",
        ));
    }
    transition::finish_exit(function, exit, state, storage, leaves, budget)
}
