//! Exact selected structural twin; complete original remains in the parent owner.
use super::*;

fn successor(
    function: &ResolvedFunction,
    block: BlockId,
    index: usize,
    budget: &mut ReplayBudget,
) -> Result<Option<BlockId>, Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a ResolvedFunction,
        BlockId,
        usize,
        &'a mut ReplayBudget,
        &'a crate::cleanup_plan::CleanupPlan,
        &'a crate::cleanup_plan::CleanupBlock,
        Option<EdgeId>,
        &'a ExitTarget,
    );
    budget.reserve_array::<Receiving<'_>>(function, 1, "selected successor local receiving")?;
    budget.charge(function, 8, "selected successor local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<Option<BlockId>>(function, budget)
    )?;
    let plan = &function.cleanup_plan;
    let item = plan
        .blocks
        .get(block.0 as usize)
        .ok_or_else(|| permit.control(Reason::Finite(Kind::CleanupCfgReferencesAnUnknownBlock)))?;
    let edge = match &item.terminator {
        CleanupTerminator::Goto(edge) => (index == 0).then_some(*edge),
        CleanupTerminator::Branch(edges) => {
            if edges.len() != 2 || edges[0] == edges[1] {
                return Err(permit.control(Reason::Finite(
                    Kind::CleanupBranchMustOwnExactlyTwoDistinctEdges,
                )));
            }
            edges.get(index).copied()
        }
        CleanupTerminator::Exit(exit) => {
            let target = plan.exits.get(exit.0 as usize).ok_or_else(|| {
                permit.control(Reason::Finite(Kind::CleanupCfgReferencesAnUnknownExit))
            })?;
            match &target.continuation {
                ExitContinuation::Continue(edge)
                | ExitContinuation::LoopBreak { edge, .. }
                | ExitContinuation::FunctionReturn { edge, .. } => (index == 0).then_some(*edge),
                ExitContinuation::LoopLatch { .. } => {
                    return Ok(if index == 0 {
                        lexical_cert::latch_resume(plan, *exit)
                    } else {
                        None
                    })
                }
                ExitContinuation::CommitResult { .. }
                | ExitContinuation::ReturnFailure { .. }
                | ExitContinuation::ReturnUnit => None,
            }
        }
    };
    edge.map(|id| {
        plan.edges
            .get(id.0 as usize)
            .map(|edge| edge.to)
            .ok_or_else(|| permit.control(Reason::Finite(Kind::CleanupCfgReferencesAnUnknownEdge)))
    })
    .transpose()
}

pub(in crate::cleanup_plan::replay) fn validate_reachable_acyclic_cfg(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a ResolvedFunction,
        &'a mut ReplayBudget,
        &'a crate::cleanup_plan::CleanupPlan,
        Vec<u8>,
        Vec<(BlockId, bool)>,
        Option<(BlockId, bool)>,
        BlockId,
        bool,
        usize,
        &'a mut u8,
        usize,
        BlockId,
        &'a u8,
        std::iter::Rev<std::ops::Range<usize>>,
    );
    budget.reserve_array::<Receiving<'_>>(
        function,
        1,
        "selected validate_reachable_acyclic_cfg local receiving",
    )?;
    budget.charge(
        function,
        14,
        "selected validate_reachable_acyclic_cfg local receiving",
    )?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    let plan = &function.cleanup_plan;
    budget.reserve_array::<u8>(function, plan.blocks.len(), "structural CFG colors")?;
    let mut colors = vec![0_u8; plan.blocks.len()];
    let mut stack = Vec::new();
    budget.reserve_vec(function, &mut stack, 1, "structural CFG entry")?;
    stack.push((plan.entry, false));
    while let Some((block, expanded)) = stack.pop() {
        budget.charge(function, 1, "structural CFG traversal")?;
        let index = block.0 as usize;
        let color = colors.get_mut(index).ok_or_else(|| {
            permit.control(Reason::Finite(Kind::CleanupCfgReferencesAnUnknownBlock))
        })?;
        if expanded {
            *color = 2;
            continue;
        }
        match *color {
            1 => return Err(permit.control(Reason::Finite(Kind::CleanupCfgContainsACycle))),
            2 => continue,
            _ => *color = 1,
        }
        budget.reserve_vec(function, &mut stack, 1, "structural CFG finish")?;
        stack.push((block, true));
        // The authenticated terminator has at most two successors. Read it
        // directly in the same reverse order as the legacy owned scratch Vec.
        for next in (0..2).rev() {
            let Some(target) = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                budget,
                function,
                (
                    &ResolvedFunction,
                    BlockId,
                    usize,
                    &mut ReplayBudget,
                    Result<Option<BlockId>, Error>
                ),
                5,
                "selected control successor call receiving",
                successor(function, block, next, budget)
            )?
            else {
                continue;
            };
            let color = colors.get(target.0 as usize).ok_or_else(|| {
                permit.control(Reason::Finite(Kind::CleanupCfgReferencesAnUnknownBlock))
            })?;
            if *color == 1 {
                return Err(permit.control(Reason::Finite(Kind::CleanupCfgContainsACycle)));
            }
            if *color == 0 {
                budget.reserve_vec(function, &mut stack, 1, "structural CFG successor")?;
                stack.push((target, false));
            }
        }
    }
    if colors.contains(&0) {
        return Err(permit.control(Reason::Finite(Kind::CleanupCfgContainsAnUnreachableBlock)));
    }
    Ok(())
}
