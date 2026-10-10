//! Execution-guard cleanup boundaries of cleanup plan v18.
//!
//! A boundary certificate selects no status, admits no owner or ABI and cannot
//! make an allocation failure recoverable. Replay reconstructs the exact
//! positions from typed HIR (function entry before `requires`, and each loop's
//! iteration entry after its condition's cleanup and before its body, for
//! ordinary and lexical loops alike) and the exact finalizer contour from the
//! replayed ownership state at that block. Metadata is never compared against a
//! builder; a forged, missing, duplicate or out-of-domain boundary refuses.
use super::lexical_cert::LexicalAuth;
use super::lexical_hir::LexicalFacts;
use super::*;
use crate::cleanup_plan::ExecutionBoundaryKind;

/// Structural authentication: kind, site, block, region and region chain.
pub(super) fn authenticate(
    function: &ResolvedFunction,
    facts: &LexicalFacts<'_>,
    domains: &[Option<usize>],
) -> Result<BTreeMap<BlockId, usize>, Diagnostic> {
    let plan = &function.cleanup_plan;
    let fail = |message: &str| replay_error(function, message.to_owned());
    let control = plan
        .lexical_control
        .as_deref()
        .ok_or_else(|| fail("execution boundaries have no lexical-control certificate"))?;
    if control.execution_boundaries.len() != facts.boundaries.len() + 1 {
        return Err(fail(
            "execution boundaries do not cover exactly the entry and every reachable loop",
        ));
    }
    let mut by_block = BTreeMap::new();
    for (index, item) in control.execution_boundaries.iter().enumerate() {
        let (kind, site, block) = if index == 0 {
            (
                ExecutionBoundaryKind::FunctionEntry,
                &function.body.id,
                plan.entry,
            )
        } else {
            let site = facts.boundaries[index - 1];
            let (_, yes, _) = super::lexical_cert::loop_branch(function, site)?;
            (
                ExecutionBoundaryKind::LoopIteration,
                site,
                plan.edges[yes.0 as usize].to,
            )
        };
        if item.kind != kind || &item.site != site || item.block != block {
            return Err(fail(
                "execution boundary kind, site or block differs from typed HIR",
            ));
        }
        let region = plan.blocks[block.0 as usize].region;
        if item.region != region
            || item.leaves_regions != region_chain_to_root(function, region)?
            || domains[block.0 as usize].is_some()
            || by_block.insert(block, index).is_some()
        {
            return Err(fail(
                "execution boundary region, contour or proof domain is not canonical",
            ));
        }
    }
    Ok(by_block)
}

/// Dataflow authentication at the boundary block: the certificate's finalizer
/// vector must be exactly the live leaves of the replayed ownership state, in
/// canonical reverse-initialization order with exact conditional case guards.
#[allow(clippy::too_many_arguments)]
pub(super) fn check(
    function: &ResolvedFunction,
    auth: &LexicalAuth,
    block: BlockId,
    state: &PathState,
    storage: &BTreeSet<StorageId>,
    storage_regions: &BTreeMap<StorageId, CleanupRegionId>,
    leaves: &Leaves,
) -> Result<(), Diagnostic> {
    let Some(index) = auth.boundaries.get(&block) else {
        return Ok(());
    };
    let item = function
        .cleanup_plan
        .lexical_control
        .as_deref()
        .and_then(|control| control.execution_boundaries.get(*index))
        .ok_or_else(|| replay_error(function, "execution boundary has no certificate"))?;
    if state.pending_failure.is_some()
        || state.selected_failure.is_some()
        || state.published
        || state.returned.is_some()
    {
        return Err(replay_error(
            function,
            "execution boundary is reached with failure, return or publication state",
        ));
    }
    validate_finalizer_actions(function, &item.finalize_in_order, storage, leaves)?;
    let expected = state.live_order.iter().rev().copied().collect::<Vec<_>>();
    let mut expected_conditional = Vec::new();
    for variant in state.conditional_variants.iter().rev() {
        let _ = storage_regions
            .get(&variant.root.storage)
            .ok_or_else(|| replay_error(function, "execution boundary variant has no region"))?;
        for (case, flags) in variant.cases.iter().rev() {
            for flag in flags.iter().rev() {
                expected_conditional.push((
                    *flag,
                    crate::cleanup_plan::VariantCaseGuard {
                        storage: variant.root.storage.clone(),
                        variant: variant.variant.clone(),
                        case: case.clone(),
                    },
                ));
            }
        }
    }
    let actual = item
        .finalize_in_order
        .iter()
        .filter_map(|action| {
            state
                .live_order
                .contains(&action.guard_flag)
                .then_some(action.guard_flag)
        })
        .collect::<Vec<_>>();
    let actual_conditional = item
        .finalize_in_order
        .iter()
        .filter_map(|action| {
            action
                .active_case
                .as_ref()
                .map(|condition| (action.guard_flag, condition.clone()))
        })
        .collect::<Vec<_>>();
    // Every action must finalize a live leaf of this exact state: a stale
    // finalizer for a dead flag would double-release at a guard failure.
    let actions = item
        .finalize_in_order
        .iter()
        .filter(|action| action.active_case.is_none())
        .count();
    if actual != expected || actual_conditional != expected_conditional || actions != expected.len()
    {
        return Err(replay_error(
            function,
            "execution boundary finalizers differ from the replayed live ownership contour",
        ));
    }
    Ok(())
}
