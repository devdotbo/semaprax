//! Actual retained lexical-builder backing; no descriptor discounts.
use super::*;

pub(in crate::cleanup_plan::build) fn flow_capacity(state: &FlowState) -> usize {
    state.live_order.capacity() * std::mem::size_of::<LivenessFlagId>()
        + state
            .renewals
            .iter()
            .map(|(at, history)| {
                identity_capacity(at)
                    + history.owned_capacity()
                    + std::mem::size_of::<(
                        ExpressionId,
                        super::super::super::renewal_history::RenewalHistory,
                    )>()
            })
            .sum::<usize>()
        + state.conditional_variants.capacity() * std::mem::size_of::<ConditionalFlowVariant>()
        + state
            .conditional_variants
            .iter()
            .map(|variant| {
                place_capacity(&variant.root)
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
                .map(|source| identity_capacity(&source.expression))
                .sum::<usize>()
    }
}

fn result_capacity(source: &CleanupResultSource) -> usize {
    match source {
        CleanupResultSource::Scalar { expression } => expression
            .owned_allocation_bytes()
            .expect("result identity capacity"),
        CleanupResultSource::Owned { storage } => place_capacity(storage),
    }
}

pub(in crate::cleanup_plan::build) fn exit_capacity(continuation: &ExitContinuation) -> usize {
    match continuation {
        // These variants predate lexical control. Their existing census is
        // unchanged for legacy callers; pending lexical route payloads use the
        // complete result_capacity above.
        ExitContinuation::CommitResult {
            source: CleanupResultSource::Scalar { expression },
        } => expression.as_str().len(),
        ExitContinuation::CommitResult {
            source: CleanupResultSource::Owned { storage },
        } => cleanup_place_owned_capacity(storage),
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

fn identity_capacity(id: &ExpressionId) -> usize {
    id.owned_allocation_bytes()
        .expect("lexical identity backing census")
}

// The legacy builder census remains byte-for-byte unchanged. New lexical
// retained places charge full expression backing, including spare capacity.
fn place_capacity(place: &CleanupPlace) -> usize {
    let storage = match &place.storage {
        StorageId::Value(id) => id.as_str().len(),
        StorageId::Temporary(id) => identity_capacity(id),
        StorageId::CallArgument {
            call,
            value_expression,
            ..
        } => identity_capacity(call) + identity_capacity(value_expression),
        StorageId::ProvisionalResult => 0,
    };
    storage
        + place.projections.capacity() * std::mem::size_of::<DeclarationId>()
        + place
            .projections
            .iter()
            .map(|id| id.as_str().len())
            .sum::<usize>()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity(text: &str, capacity: usize) -> ExpressionId {
        let mut backing = String::with_capacity(capacity);
        backing.push_str(text);
        ExpressionId::from_untrimmed_backing_for_test(backing)
    }
    fn flow() -> FlowState {
        FlowState {
            live_order: Vec::new(),
            renewals: BTreeMap::new(),
            conditional_variants: Vec::new(),
        }
    }
    fn lexical() -> LexicalBuildState {
        LexicalBuildState {
            owner: FunctionExecutionId::Monomorphic(DeclarationId::new("capacity.owner")),
            certificate: crate::cleanup_plan::LexicalControlPlan {
                schema: "semaprax.lexical-control-flow.v1",
                loops: Vec::new(),
                exits: Vec::new(),
                resumes: Vec::new(),
                abrupt_only_regions: Vec::new(),
                function_success_exits: Vec::new(),
                execution_boundaries: Vec::new(),
            },
            returns: Vec::new(),
            active_loops: Vec::new(),
            wrapper_breaks: Vec::new(),
            building_resume: false,
            replicated_statuses: BTreeSet::new(),
        }
    }

    #[test]
    fn lexical_flow_counts_full_identity_backing_and_spare_owner_histories() {
        let mut state = flow();
        state.live_order = Vec::with_capacity(11);
        let at = identity("same.text", 179);
        let mut history = Vec::with_capacity(23);
        history.push(LivenessFlagId(5));
        let retained = crate::cleanup_plan::renewal_history::RenewalHistory::new(
            history,
            Some(LivenessFlagId(5)),
        );
        let expected = state.live_order.capacity() * std::mem::size_of::<LivenessFlagId>()
            + at.owned_allocation_bytes().unwrap()
            + retained.owned_capacity()
            + std::mem::size_of::<(
                ExpressionId,
                crate::cleanup_plan::renewal_history::RenewalHistory,
            )>();
        state.renewals.insert(at, retained);
        assert_eq!(flow_capacity(&state), expected);
        let legacy_scalar = ExitContinuation::CommitResult {
            source: CleanupResultSource::Scalar {
                expression: identity("legacy.result", 197),
            },
        };
        assert_eq!(exit_capacity(&legacy_scalar), "legacy.result".len());
        let legacy_failure = ExitContinuation::ReturnFailure {
            source: StatusSourceId {
                expression: identity("legacy.failure", 199),
                lane: crate::cleanup_plan::StatusLane::OperationFailure,
            },
        };
        assert_eq!(exit_capacity(&legacy_failure), "legacy.failure".len());
        let root = identity("conditional.root", 211);
        let mut flags = Vec::with_capacity(29);
        flags.push(LivenessFlagId(7));
        let mut cases = Vec::with_capacity(13);
        cases.push((DeclarationId::new("case"), flags));
        let mut projections = Vec::with_capacity(17);
        projections.push(DeclarationId::new("field"));
        let place = CleanupPlace {
            storage: StorageId::Temporary(root),
            projections,
        };
        let payload = place_capacity(&place)
            + "variant".len()
            + cases.capacity() * std::mem::size_of::<(DeclarationId, Vec<LivenessFlagId>)>()
            + "case".len()
            + cases[0].1.capacity() * std::mem::size_of::<LivenessFlagId>();
        state.conditional_variants = Vec::with_capacity(19);
        state.conditional_variants.push(ConditionalFlowVariant {
            root: place,
            variant: DeclarationId::new("variant"),
            cases,
        });
        assert_eq!(
            flow_capacity(&state),
            expected
                + payload
                + state.conditional_variants.capacity()
                    * std::mem::size_of::<ConditionalFlowVariant>()
        );
    }

    #[test]
    fn lexical_builder_counts_empty_spare_domains_and_retained_route_payloads() {
        let mut state = lexical();
        let initial = state.owned_capacity();
        state.returns = Vec::with_capacity(7);
        state.active_loops = Vec::with_capacity(11);
        state.wrapper_breaks = Vec::with_capacity(13);
        state.certificate.resumes = Vec::with_capacity(17);
        let reserved = state.returns.capacity() * std::mem::size_of::<PendingReturn>()
            + state.active_loops.capacity() * std::mem::size_of::<LoopContext>()
            + state.wrapper_breaks.capacity()
                * std::mem::size_of::<crate::cleanup_plan::build::lexical_break::PendingWrapperBreak>(
                )
            + state.certificate.resumes.capacity()
                * std::mem::size_of::<crate::cleanup_plan::ResumeCertificate>();
        assert_eq!(state.owned_capacity(), initial + reserved);
        let result = identity("return.result", 233);
        let pending_state = flow();
        let pending_bytes =
            result.owned_allocation_bytes().unwrap() + flow_capacity(&pending_state);
        state.returns.push(PendingReturn {
            entry: BlockId(0),
            state: pending_state,
            result: CleanupResultSource::Scalar { expression: result },
        });
        let source = StatusSourceId {
            expression: identity("failure.source", 251),
            lane: crate::cleanup_plan::StatusLane::OperationFailure,
        };
        let status_bytes = source.expression.owned_allocation_bytes().unwrap()
            + std::mem::size_of::<StatusSourceId>()
            + std::mem::size_of::<BTreeSet<StatusSourceId>>();
        state.replicated_statuses.insert(source);
        let mut blocks = Vec::with_capacity(31);
        blocks.push(BlockId(0));
        let blocks_bytes = blocks.capacity() * std::mem::size_of::<BlockId>();
        state
            .certificate
            .resumes
            .push(crate::cleanup_plan::ResumeCertificate {
                entry: BlockId(0),
                blocks,
            });
        assert_eq!(
            state.owned_capacity(),
            initial + reserved + pending_bytes + status_bytes + blocks_bytes
        );
        let target = crate::hir::ControlTarget::Loop {
            owner: state.owner.clone(),
            site: identity("loop.condition", 277),
        };
        let body = identity("loop.body", 293);
        let latch = crate::hir::LoopLatchKind::SourceForStep {
            kind: crate::hir::SourceForKind::Copy,
            step: identity("loop.advance", 307),
        };
        let entry = flow();
        let context_bytes =
            crate::private_capacity_contract::control_target_owned_capacity(&target).unwrap()
                + body.owned_allocation_bytes().unwrap()
                + crate::private_capacity_contract::latch_owned_capacity(&latch).unwrap()
                + flow_capacity(&entry);
        state.active_loops.push(LoopContext {
            target,
            region: CleanupRegionId(0),
            body,
            latch,
            entry,
            after: BlockId(0),
        });
        assert_eq!(
            state.owned_capacity(),
            initial + reserved + pending_bytes + status_bytes + blocks_bytes + context_bytes
        );
    }
}
