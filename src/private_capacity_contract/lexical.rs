//! Physical backing census for rare lexical-control metadata. Inline enum
//! storage is charged by its owning carrier; identities are charged per occurrence.
use crate::cleanup_plan::{
    BlockId, LexicalControlPlan, LexicalExitCertificate, LoopCertificate, ResumeCertificate,
};
use crate::hir::{ControlTarget, FunctionExecutionId, LoopLatchKind};

pub(crate) fn control_target_owned_capacity(target: &ControlTarget) -> Option<usize> {
    fn owner_bytes(owner: &FunctionExecutionId) -> usize {
        match owner {
            FunctionExecutionId::Monomorphic(id) => id.as_str().len(),
            FunctionExecutionId::Generic(id) => id.as_str().len(),
        }
    }
    match target {
        ControlTarget::Function(owner) => Some(owner_bytes(owner)),
        ControlTarget::Loop { owner, site } => {
            owner_bytes(owner).checked_add(site.owned_allocation_bytes()?)
        }
    }
}

pub(crate) fn latch_owned_capacity(latch: &LoopLatchKind) -> Option<usize> {
    match latch {
        LoopLatchKind::Condition => Some(0),
        LoopLatchKind::SourceForStep { step, .. } => step.owned_allocation_bytes(),
        LoopLatchKind::OwnedIteratorStep {
            next,
            assignment,
            step,
            remainder,
        } => next
            .owned_allocation_bytes()?
            .checked_add(assignment.owned_allocation_bytes()?)?
            .checked_add(step.as_str().len())?
            .checked_add(remainder.as_str().len()),
    }
}

pub(super) fn plan_owned_capacity(control: Option<&LexicalControlPlan>) -> Option<usize> {
    let Some(control) = control else {
        return Some(0);
    };
    let mut bytes = std::mem::size_of::<LexicalControlPlan>()
        .checked_add(
            control
                .loops
                .capacity()
                .checked_mul(std::mem::size_of::<LoopCertificate>())?,
        )?
        .checked_add(
            control
                .exits
                .capacity()
                .checked_mul(std::mem::size_of::<LexicalExitCertificate>())?,
        )?
        .checked_add(
            control
                .resumes
                .capacity()
                .checked_mul(std::mem::size_of::<ResumeCertificate>())?,
        )?;
    bytes = bytes
        .checked_add(
            control
                .abrupt_only_regions
                .capacity()
                .checked_mul(std::mem::size_of::<crate::cleanup_plan::CleanupRegionId>())?,
        )?
        .checked_add(
            control
                .function_success_exits
                .capacity()
                .checked_mul(std::mem::size_of::<crate::cleanup_plan::ExitTargetId>())?,
        )?;
    bytes = bytes.checked_add(control.execution_boundaries.capacity().checked_mul(
        std::mem::size_of::<crate::cleanup_plan::ExecutionBoundaryCertificate>(),
    )?)?;
    for boundary in &control.execution_boundaries {
        bytes = bytes
            .checked_add(boundary.site.owned_allocation_bytes()?)?
            .checked_add(
                boundary
                    .leaves_regions
                    .capacity()
                    .checked_mul(std::mem::size_of::<crate::cleanup_plan::CleanupRegionId>())?,
            )?
            .checked_add(
                boundary
                    .finalize_in_order
                    .capacity()
                    .checked_mul(std::mem::size_of::<crate::cleanup_plan::FinalizeAction>())?,
            )?;
        for action in &boundary.finalize_in_order {
            bytes = bytes
                .checked_add(super::cleanup_place_owned_capacity(&action.source)?)?
                .checked_add(action.lifecycle_id.as_str().len())?;
            if let Some(guard) = &action.active_case {
                bytes = bytes
                    .checked_add(super::storage_owned_capacity(&guard.storage)?)?
                    .checked_add(guard.variant.as_str().len())?
                    .checked_add(guard.case.as_str().len())?;
            }
        }
    }
    for item in &control.loops {
        bytes = bytes
            .checked_add(control_target_owned_capacity(&item.target)?)?
            .checked_add(item.condition.owned_allocation_bytes()?)?
            .checked_add(item.body.owned_allocation_bytes()?)?
            .checked_add(latch_owned_capacity(&item.latch)?)?
            .checked_add(
                item.entry_order
                    .capacity()
                    .checked_mul(std::mem::size_of::<crate::cleanup::LivenessFlagId>())?,
            )?;
    }
    for item in &control.exits {
        bytes = bytes
            .checked_add(item.site.owned_allocation_bytes()?)?
            .checked_add(control_target_owned_capacity(&item.target)?)?;
    }
    for item in &control.resumes {
        bytes = bytes.checked_add(
            item.blocks
                .capacity()
                .checked_mul(std::mem::size_of::<BlockId>())?,
        )?;
    }
    Some(bytes)
}
