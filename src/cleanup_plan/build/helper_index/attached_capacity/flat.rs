//! Bound every flat carrier visited by the subsequent physical census, before
//! that census runs. This does not validate or grant attached plan authority.
use super::*;
use crate::cleanup_plan::{CleanupTerminator, FinalizeAction, StatusProducer};
fn place(place: &CleanupPlace, work: &mut usize) -> Result<(), Diagnostic> {
    charge(work, place.projections.len())
}
fn actions(actions: &[FinalizeAction], work: &mut usize) -> Result<(), Diagnostic> {
    charge(work, actions.len())?;
    for action in actions {
        place(&action.source, work)?;
    }
    Ok(())
}
fn result(
    source: &crate::cleanup_plan::CleanupResultSource,
    work: &mut usize,
) -> Result<(), Diagnostic> {
    if let crate::cleanup_plan::CleanupResultSource::Owned { storage } = source {
        place(storage, work)?;
    }
    Ok(())
}
pub(super) fn plan(plan: &CleanupPlan, work: &mut usize) -> Result<(), Diagnostic> {
    for count in [
        plan.slots.len(),
        plan.blocks.len(),
        plan.edges.len(),
        plan.regions.len(),
        plan.exits.len(),
        plan.status_sources.len(),
        plan.entry_state.live_owned_parameters.len(),
        plan.entry_state.conditional_owned_parameters.len(),
    ] {
        charge(work, count)?;
    }
    for item in &plan.entry_state.live_owned_parameters {
        place(item, work)?;
    }
    for item in &plan.entry_state.conditional_owned_parameters {
        charge(work, item.cases.len())?;
        for case in &item.cases {
            charge(work, case.live_places.len())?;
            for item in &case.live_places {
                place(item, work)?;
            }
        }
    }
    for item in &plan.status_sources {
        if let StatusProducer::CheckedArithmetic {
            normalized_cases, ..
        } = &item.producer
        {
            charge(work, normalized_cases.len())?;
        }
    }
    for block in &plan.blocks {
        charge(work, block.transitions.len())?;
        for item in &block.transitions {
            match item {
                CleanupTransition::ReserveRenewal { binding, .. } => place(binding, work)?,
                CleanupTransition::Initialize { destination, .. }
                | CleanupTransition::InitializeVariant { destination, .. } => {
                    place(destination, work)?
                }
                CleanupTransition::Renew {
                    source,
                    destination,
                    ..
                }
                | CleanupTransition::Transfer {
                    source,
                    destination,
                    ..
                }
                | CleanupTransition::TransferVariant {
                    source,
                    destination,
                    ..
                } => {
                    place(source, work)?;
                    place(destination, work)?;
                }
                CleanupTransition::AuthenticateVariantCase { source, .. } => place(source, work)?,
                CleanupTransition::CallCommit { arguments, .. } => {
                    charge(work, arguments.len())?;
                    for argument in arguments {
                        place(&argument.source, work)?;
                    }
                }
                CleanupTransition::SelectFailure { .. }
                | CleanupTransition::StageCopyResult { .. } => {}
            }
        }
        if let CleanupTerminator::Branch(edges) = &block.terminator {
            charge(work, edges.len())?;
        }
    }
    for region in &plan.regions {
        charge(work, region.slots.len())?;
    }
    for exit in &plan.exits {
        charge(work, exit.leaves_regions.len())?;
        actions(&exit.finalize_in_order, work)?;
        match &exit.continuation {
            crate::cleanup_plan::ExitContinuation::FunctionReturn { source, .. }
            | crate::cleanup_plan::ExitContinuation::CommitResult { source } => {
                result(source, work)?
            }
            crate::cleanup_plan::ExitContinuation::Continue(_)
            | crate::cleanup_plan::ExitContinuation::LoopLatch { .. }
            | crate::cleanup_plan::ExitContinuation::LoopBreak { .. }
            | crate::cleanup_plan::ExitContinuation::ReturnFailure { .. }
            | crate::cleanup_plan::ExitContinuation::ReturnUnit => {}
        }
    }
    if let Some(control) = &plan.lexical_control {
        for count in [
            control.loops.len(),
            control.exits.len(),
            control.resumes.len(),
            control.abrupt_only_regions.len(),
            control.function_success_exits.len(),
            control.execution_boundaries.len(),
        ] {
            charge(work, count)?;
        }
        for item in &control.loops {
            charge(work, item.entry_order.len())?;
        }
        for item in &control.resumes {
            charge(work, item.blocks.len())?;
        }
        for item in &control.execution_boundaries {
            charge(work, item.leaves_regions.len())?;
            actions(&item.finalize_in_order, work)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn flat_attached_region_inventory_is_charged_at_exact_and_one_short_work() {
        let ast = crate::check("module flat;@id(\"main\") fn main()->i64{0}", "flat.spx").unwrap();
        let resolved = crate::hir::resolve(&ast).unwrap();
        let mut attached = resolved.functions[0].cleanup_plan.clone();
        assert!(attached.slots.is_empty());
        let mut remaining = crate::cache_codec::MAX_NODES;
        plan(&attached, &mut remaining).unwrap();
        let original = crate::cache_codec::MAX_NODES - remaining;
        // This carrier was omitted by the old no-recursive-shape fast path.
        attached.regions[0]
            .slots
            .extend([crate::cleanup_plan::StorageId::ProvisionalResult; 3]);
        let mut exact = original + 3;
        plan(&attached, &mut exact).unwrap();
        assert_eq!(exact, 0);
        let mut short = original + 2;
        let error = plan(&attached, &mut short).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(error.message.contains("census work exhausted"));
    }
}
