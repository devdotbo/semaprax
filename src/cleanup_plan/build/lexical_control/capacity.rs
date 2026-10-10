//! Actual retained lexical-builder backing; no descriptor discounts.
use super::*;

pub(in crate::cleanup_plan::build) fn flow_capacity(state: &FlowState) -> usize {
    flow_state_owned_capacity(state)
        + state.conditional_variants.capacity() * std::mem::size_of::<ConditionalFlowVariant>()
        + state
            .conditional_variants
            .iter()
            .map(|variant| {
                cleanup_place_owned_capacity(&variant.root)
                    + variant.variant.as_str().len()
                    + variant.cases.capacity()
                        * std::mem::size_of::<(DeclarationId, Vec<LivenessFlagId>)>()
                    + variant
                        .cases
                        .iter()
                        .map(|(case, flags)| {
                            case.as_str().len()
                                + flags.capacity() * std::mem::size_of::<LivenessFlagId>()
                        })
                        .sum::<usize>()
            })
            .sum::<usize>()
}

impl LexicalBuildState {
    pub(in crate::cleanup_plan::build) fn owned_capacity(&self) -> usize {
        let owner = match &self.owner {
            FunctionExecutionId::Monomorphic(id) => id.as_str().len(),
            FunctionExecutionId::Generic(id) => id.as_str().len(),
        };
        std::mem::size_of::<Self>()
            + owner
            + crate::private_capacity_contract::lexical_plan_owned_capacity(Some(&self.certificate))
                .expect("lexical retained capacity fits")
            - std::mem::size_of::<super::super::super::LexicalControlPlan>()
            + self.returns.capacity() * std::mem::size_of::<PendingReturn>()
            + self
                .returns
                .iter()
                .map(|pending| flow_capacity(&pending.state) + result_capacity(&pending.result))
                .sum::<usize>()
            + self.active_loops.capacity() * std::mem::size_of::<LoopContext>()
            + self
                .active_loops
                .iter()
                .map(|context| {
                    crate::private_capacity_contract::control_target_owned_capacity(&context.target)
                        .expect("target capacity")
                        + context
                            .body
                            .owned_allocation_bytes()
                            .expect("body identity capacity")
                        + crate::private_capacity_contract::latch_owned_capacity(&context.latch)
                            .expect("latch capacity")
                        + flow_capacity(&context.entry)
                })
                .sum::<usize>()
            + self.wrapper_breaks.capacity()
                * std::mem::size_of::<super::super::lexical_break::PendingWrapperBreak>()
            + self
                .wrapper_breaks
                .iter()
                .map(|item| item.owned_capacity())
                .sum::<usize>()
            + self.replicated_statuses.len()
                * (std::mem::size_of::<StatusSourceId>()
                    + std::mem::size_of::<BTreeSet<StatusSourceId>>())
            + self
                .replicated_statuses
                .iter()
                .map(status_source_id_owned_capacity)
                .sum::<usize>()
    }
}

fn result_capacity(source: &CleanupResultSource) -> usize {
    match source {
        CleanupResultSource::Scalar { expression } => expression
            .owned_allocation_bytes()
            .expect("result identity capacity"),
        CleanupResultSource::Owned { storage } => cleanup_place_owned_capacity(storage),
    }
}

pub(in crate::cleanup_plan::build) fn exit_capacity(continuation: &ExitContinuation) -> usize {
    match continuation {
        ExitContinuation::CommitResult { source } => result_capacity(source),
        ExitContinuation::ReturnFailure { source } => status_source_id_owned_capacity(source),
        ExitContinuation::Continue(_) | ExitContinuation::ReturnUnit => 0,
        ExitContinuation::LoopLatch { target } | ExitContinuation::LoopBreak { target, .. } => {
            crate::private_capacity_contract::control_target_owned_capacity(target)
                .expect("target capacity")
        }
        ExitContinuation::FunctionReturn { target, source, .. } => {
            crate::private_capacity_contract::control_target_owned_capacity(target)
                .expect("target capacity")
                + result_capacity(source)
        }
    }
}
