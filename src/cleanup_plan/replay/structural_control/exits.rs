use super::*;
pub(in crate::cleanup_plan::replay) fn validate_exits(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    return_operands: &BTreeSet<ExpressionId>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let plan = &function.cleanup_plan;

    let mut continuation_edges = BTreeSet::new();
    for (index, exit) in plan.exits.iter().enumerate() {
        budget.charge(
            function,
            exit.leaves_regions.len().saturating_add(1),
            "structural exit regions",
        )?;
        if plan.lexical_control.is_none()
            && matches!(
                exit.continuation,
                ExitContinuation::LoopLatch { .. }
                    | ExitContinuation::LoopBreak { .. }
                    | ExitContinuation::FunctionReturn { .. }
            )
        {
            return Err(replay_error(
                function,
                "lexical exit continuation has no lexical-control certificate",
            ));
        }
        if exit.id.0 != u32_index(function, index, "cleanup exit")?
            || !block_exists(function, exit.from)
        {
            return Err(replay_error(
                function,
                "cleanup exit has a non-contiguous ID or unknown source block",
            ));
        }
        if !regions_match(function, exit, budget)? {
            return Err(replay_error(
                function,
                "cleanup exit does not leave the exact source-to-target region chain",
            ));
        }
        let mut prior: Option<super::CleanupRegionId> = None;
        for region in &exit.leaves_regions {
            let Some(item) = plan.regions.get(region.0 as usize) else {
                return Err(replay_error(
                    function,
                    "cleanup exit references unknown region",
                ));
            };
            if let Some(child) = prior {
                if plan.regions[child.0 as usize].parent != Some(*region) {
                    return Err(replay_error(
                        function,
                        "cleanup exit regions are not a parent chain",
                    ));
                }
            }
            prior = Some(item.id);
        }
        lexical_cert::storage::finalizer_actions(
            function,
            &exit.finalize_in_order,
            storage,
            leaves,
            budget,
        )?;
        match &exit.continuation {
            ExitContinuation::Continue(edge) => {
                owned_edge(function, exit.from, *edge, &mut continuation_edges, budget)?;
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always) {
                    return Err(replay_error(
                        function,
                        "cleanup continuation edge is conditional",
                    ));
                }
            }
            ExitContinuation::CommitResult { source } => result_source(
                program,
                function,
                source,
                storage,
                leaves,
                return_operands,
                budget,
            )?,
            ExitContinuation::FunctionReturn { edge, source, .. } => {
                owned_edge(function, exit.from, *edge, &mut continuation_edges, budget)?;
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always) {
                    return Err(replay_error(
                        function,
                        "function-return continuation edge is conditional",
                    ));
                }
                result_source(
                    program,
                    function,
                    source,
                    storage,
                    leaves,
                    return_operands,
                    budget,
                )?
            }
            ExitContinuation::LoopBreak { edge, .. } => {
                owned_edge(function, exit.from, *edge, &mut continuation_edges, budget)?;
                if !matches!(plan.edges[edge.0 as usize].condition, EdgeCondition::Always) {
                    return Err(replay_error(
                        function,
                        "loop-break continuation edge is conditional",
                    ));
                }
            }
            ExitContinuation::LoopLatch { .. } => {}
            ExitContinuation::ReturnFailure { source } => {
                budget.charge(
                    function,
                    plan.status_sources.len(),
                    "structural failure status lookup",
                )?;
                if !plan
                    .status_sources
                    .iter()
                    .any(|candidate| &candidate.id == source)
                {
                    return Err(replay_error(
                        function,
                        "cleanup reference names an unknown status source",
                    ));
                }
                budget.charge(
                    function,
                    plan.blocks[exit.from.0 as usize].transitions.len(),
                    "structural failure selection",
                )?;
                let selected = plan.blocks[exit.from.0 as usize]
                    .transitions
                    .iter()
                    .filter_map(|transition| match transition {
                        CleanupTransition::SelectFailure { source } => Some(source),
                        _ => None,
                    });
                let mut selected = selected;
                if selected.next() != Some(source) || selected.next().is_some() {
                    return Err(replay_error(
                        function,
                        "failure exit does not return its uniquely selected status",
                    ));
                }
            }
            ExitContinuation::ReturnUnit => {
                return Err(replay_error(
                    function,
                    "ReturnUnit is invalid for current source-function return types",
                ));
            }
        }
    }
    for region in &plan.regions {
        if plan.lexical_control.is_some() && region.normal_scope_end == UNRESOLVED_NORMAL_END {
            // Authenticated as abrupt-only by the lexical certificate replay.
            continue;
        }
        let exit = &plan.exits[region.normal_scope_end.0 as usize];
        if !exit.leaves_regions.contains(&region.id) {
            return Err(replay_error(
                function,
                "region normal-scope exit does not leave that region",
            ));
        }
    }
    Ok(())
}

fn owned_edge(
    function: &ResolvedFunction,
    owner: BlockId,
    edge: EdgeId,
    referenced: &mut BTreeSet<EdgeId>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let item = function
        .cleanup_plan
        .edges
        .get(edge.0 as usize)
        .ok_or_else(|| replay_error(function, "cleanup terminator references unknown edge"))?;
    if item.id != edge || item.from != owner {
        return Err(replay_error(
            function,
            "cleanup edge has the wrong owner or is referenced repeatedly",
        ));
    }
    if !insert_copy(
        function,
        budget,
        referenced,
        edge,
        "structural continuation edges",
    )? {
        return Err(replay_error(
            function,
            "cleanup edge has the wrong owner or is referenced repeatedly",
        ));
    }
    Ok(())
}

fn ancestor_contains(
    function: &ResolvedFunction,
    start: CleanupRegionId,
    wanted: CleanupRegionId,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    let mut current = Some(start);
    let mut remaining = function.cleanup_plan.regions.len();
    let mut found = false;
    while let Some(region) = current {
        budget.charge(function, 1, "structural target ancestry")?;
        remaining = remaining
            .checked_sub(1)
            .ok_or_else(|| replay_error(function, "cleanup region tree contains a cycle"))?;
        let item = function
            .cleanup_plan
            .regions
            .get(region.0 as usize)
            .ok_or_else(|| {
                replay_error(
                    function,
                    "cleanup region chain references an unknown region",
                )
            })?;
        found |= region == wanted;
        current = item.parent;
    }
    Ok(found)
}

/// Compare the complete exact leave prefix directly. No expected region chain
/// or ancestor tree is constructed; every current parent link is still checked.
pub(super) fn regions_match(
    function: &ResolvedFunction,
    exit: &ExitTarget,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    let plan = &function.cleanup_plan;
    let source = plan
        .blocks
        .get(exit.from.0 as usize)
        .ok_or_else(|| replay_error(function, "cleanup exit has an unknown source block"))?
        .region;
    let mut latch = None;
    let mut target = None;
    match &exit.continuation {
        ExitContinuation::LoopLatch {
            target: control_target,
        } => {
            if plan.lexical_control.as_deref().is_some_and(|control| {
                control.loops.iter().any(|lp| {
                    lp.target == *control_target
                        && matches!(
                            &lp.latch,
                            crate::hir::LoopLatchKind::OwnedIteratorStep { .. }
                        )
                })
            }) {
                return Err(replay_error(
                    function,
                    "selected primitive exits do not admit owned iterator scopes",
                ));
            }
            let region = lexical_cert::loop_region(plan, control_target).ok_or_else(|| {
                replay_error(function, "loop latch names an unknown lexical loop")
            })?;
            if !ancestor_contains(function, source, region, budget)? {
                return Err(replay_error(
                    function,
                    "loop latch exit does not originate inside its loop region",
                ));
            }
            latch = Some(region);
        }
        ExitContinuation::Continue(edge)
        | ExitContinuation::LoopBreak { edge, .. }
        | ExitContinuation::FunctionReturn { edge, .. } => {
            target = Some(
                plan.edges
                    .get(edge.0 as usize)
                    .and_then(|edge| plan.blocks.get(edge.to.0 as usize))
                    .ok_or_else(|| {
                        replay_error(function, "cleanup continuation has an unknown target")
                    })?
                    .region,
            );
        }
        ExitContinuation::CommitResult { .. }
        | ExitContinuation::ReturnFailure { .. }
        | ExitContinuation::ReturnUnit => {}
    }
    let mut current = Some(source);
    let mut remaining = plan.regions.len();
    let mut index = 0usize;
    let mut stopped = false;
    while let Some(region) = current {
        budget.charge(function, 1, "structural source ancestry")?;
        remaining = remaining
            .checked_sub(1)
            .ok_or_else(|| replay_error(function, "cleanup region tree contains a cycle"))?;
        let item = plan.regions.get(region.0 as usize).ok_or_else(|| {
            replay_error(
                function,
                "cleanup region chain references an unknown region",
            )
        })?;
        stopped |= latch == Some(region);
        if let Some(target) = target {
            stopped |= ancestor_contains(function, target, region, budget)?;
        }
        if !stopped {
            if exit.leaves_regions.get(index) != Some(&region) {
                return Ok(false);
            }
            index += 1;
        }
        current = item.parent;
    }
    Ok(index == exit.leaves_regions.len())
}

fn result_source(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    source: &CleanupResultSource,
    storage: &BTreeSet<StorageId>,
    leaves: &Leaves,
    return_operands: &BTreeSet<ExpressionId>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if matches!(function.return_type, ResolvedType::Nominal { .. }) {
        super::super::selected_profile::collections::prepay(
            program,
            function,
            super::super::selected_profile::collections::type_key_bytes(&function.return_type)?,
            budget,
        )?;
    }
    let owned = crate::cleanup::selected_inventory_needs_drop_in(program, &function.return_type)?;
    match source {
        CleanupResultSource::Scalar { expression } => {
            if owned {
                return Err(replay_error(
                    function,
                    "droppable function result uses a scalar result commit",
                ));
            }
            if expression != &function.body.id && !return_operands.contains(expression) {
                return Err(replay_error(
                    function,
                    "scalar result commit does not publish the function body",
                ));
            }
        }
        CleanupResultSource::Owned { storage: result } => {
            if !owned
                || result.storage != StorageId::ProvisionalResult
                || !result.projections.is_empty()
            {
                return Err(replay_error(
                    function,
                    "owned result commit must publish the whole droppable provisional result",
                ));
            }
            path_join::storage::validate_place(function, result, storage, leaves, budget)?;
            if !leaves
                .values()
                .any(|leaf| leaf.place.storage == StorageId::ProvisionalResult)
            {
                return Err(replay_error(
                    function,
                    "owned result commit has no provisional-result leaves",
                ));
            }
        }
    }
    Ok(())
}
