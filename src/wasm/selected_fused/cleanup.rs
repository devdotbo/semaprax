//! Canonical cleanup replay for the selected lowering.
//!
//! The function's attached `CleanupPlan` was authenticated as the exact
//! canonical plan before lowering. Each owned leaf of each plan slot gets one
//! i64 carrier local (the host handle) and one i32 liveness local (the plan's
//! guard flag). Transitions are replayed in plan vector order at the
//! expression that the plan names. Every exit emits its `finalize_in_order`
//! vector verbatim, guarded by exactly the plan's flag; nothing is sorted,
//! merged or repaired. After lowering, a census requires every transition and
//! every reachable exit to have been reproduced, so a plan the lowering does
//! not model refuses instead of being approximated.
use super::lower::{unsupported, Lower};
use super::{import, Refusal};
use crate::cleanup::{
    FieldLivenessShape, LivenessFlagId, BYTES_DROP_LIFECYCLE_ID, STRING_DROP_LIFECYCLE_ID,
    VEC_DROP_LIFECYCLE_ID,
};
use crate::cleanup_plan::{
    CheckedOperation, CleanupPlace, CleanupPlan, CleanupRegionId, CleanupResultSource,
    CleanupTransition as Tr, ExitContinuation, FinalizeAction, StatusLane, StatusProducer,
    StatusSourceId, StorageId,
};
use crate::hir::{DeclarationId, ExpressionId, ResolvedExpr as E, ResolvedExprKind as K};
use wasm_encoder::{BlockType, Instruction as I, ValType};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Lifecycle {
    Vector,
    Text,
    Payload,
}

struct Leaf {
    place: CleanupPlace,
    flag_id: LivenessFlagId,
    lifecycle: Lifecycle,
    carrier: u32,
    flag: u32,
}

/// Owner handles produced by one lowered expression, for `Initialize` and
/// `Renew` destinations and for the renewed handle of a `Transfer`.
pub(super) enum Owned {
    None,
    Leaf(u32),
    Record(Vec<(DeclarationId, u32)>),
}

/// What a failure exit reports, checked against the plan's status producer.
#[derive(Clone, Copy)]
pub(super) enum Failure {
    VecAllocation,
    VecPush,
    /// An authenticated row field read past the Vec length.
    VecRead,
    Arithmetic(CheckedOperation),
    Conversion,
}

pub(super) struct Replay<'a> {
    plan: &'a CleanupPlan,
    leaves: Vec<Leaf>,
    applied: Vec<Vec<bool>>,
    emitted: Vec<bool>,
}

impl<'a> Replay<'a> {
    pub(super) fn empty(plan: &'a CleanupPlan) -> Self {
        Self {
            plan,
            leaves: Vec::new(),
            applied: plan
                .blocks
                .iter()
                .map(|block| vec![false; block.transitions.len()])
                .collect(),
            emitted: vec![false; plan.exits.len()],
        }
    }
}

fn flatten(
    storage: &StorageId,
    shape: &FieldLivenessShape,
    projections: &mut Vec<DeclarationId>,
    visit: &mut impl FnMut(CleanupPlace, LivenessFlagId, &DeclarationId) -> Result<(), Refusal>,
) -> Result<(), Refusal> {
    match shape {
        FieldLivenessShape::NoDrop => Ok(()),
        FieldLivenessShape::Leaf { flag, lifecycle } => visit(
            CleanupPlace {
                storage: storage.clone(),
                projections: projections.clone(),
            },
            *flag,
            lifecycle,
        ),
        FieldLivenessShape::Record { fields, .. } => {
            for field in fields {
                projections.push(field.field.clone());
                flatten(storage, &field.shape, projections, visit)?;
                projections.pop();
            }
            Ok(())
        }
        FieldLivenessShape::Variant { .. } => Err(Refusal::Cleanup),
    }
}

impl<'a> Lower<'a> {
    /// Allocate every leaf's carrier and flag. Refuses lifecycles, entry
    /// state and transition kinds outside the closed selected profile.
    pub(super) fn replay_init(&mut self) -> Result<(), Refusal> {
        let plan = self.cleanup.plan;
        if !plan.entry_state.live_owned_parameters.is_empty()
            || !plan.entry_state.conditional_owned_parameters.is_empty()
            || plan.lexical_control.is_some()
        {
            return Err(Refusal::Cleanup);
        }
        for block in &plan.blocks {
            for transition in &block.transitions {
                if !matches!(
                    transition,
                    Tr::Initialize { .. }
                        | Tr::Renew { .. }
                        | Tr::Transfer { .. }
                        | Tr::ReserveRenewal { .. }
                        | Tr::CallCommit { .. }
                        | Tr::SelectFailure { .. }
                ) {
                    return Err(Refusal::Cleanup);
                }
            }
        }
        let selected = self.selected;
        let vector = &selected.vector.ty;
        let mut found = Vec::new();
        for slot in &plan.slots {
            flatten(
                &slot.storage,
                &slot.field_liveness_shape,
                &mut Vec::new(),
                &mut |place, flag, lifecycle| {
                    let lifecycle = match lifecycle.as_str() {
                        VEC_DROP_LIFECYCLE_ID
                            if &slot.ty == vector && place.projections.is_empty() =>
                        {
                            Lifecycle::Vector
                        }
                        STRING_DROP_LIFECYCLE_ID => Lifecycle::Text,
                        BYTES_DROP_LIFECYCLE_ID => Lifecycle::Payload,
                        _ => return Err(Refusal::Cleanup),
                    };
                    found.push((place, flag, lifecycle));
                    Ok(())
                },
            )?;
        }
        for (place, flag_id, lifecycle) in found {
            if self
                .cleanup
                .leaves
                .iter()
                .any(|leaf| leaf.place == place || leaf.flag_id == flag_id)
            {
                return Err(Refusal::Cleanup);
            }
            let carrier = self.local(ValType::I64);
            let flag = self.local(ValType::I32);
            self.cleanup.leaves.push(Leaf {
                place,
                flag_id,
                lifecycle,
                carrier,
                flag,
            });
        }
        Ok(())
    }

    fn leaves_under(&self, place: &CleanupPlace) -> Vec<usize> {
        self.cleanup
            .leaves
            .iter()
            .enumerate()
            .filter(|(_, leaf)| {
                leaf.place.storage == place.storage
                    && leaf.place.projections.starts_with(&place.projections)
            })
            .map(|(index, _)| index)
            .collect()
    }

    /// Carrier of the whole-value leaf of a selected Vec binding.
    pub(super) fn vector_carrier(&self, binding: &crate::hir::ValueId) -> Result<u32, Refusal> {
        let place = CleanupPlace {
            storage: StorageId::Value(binding.clone()),
            projections: Vec::new(),
        };
        match self.leaves_under(&place).as_slice() {
            [index] if self.cleanup.leaves[*index].lifecycle == Lifecycle::Vector => {
                Ok(self.cleanup.leaves[*index].carrier)
            }
            _ => Err(Refusal::Cleanup),
        }
    }

    fn set(&mut self, local: u32, value: i32) {
        self.op(I::I32Const(value));
        self.op(I::LocalSet(local));
    }

    /// Bind produced handles to every leaf under `destination` and mark them live.
    fn assign(&mut self, destination: &CleanupPlace, value: &Owned) -> Result<(), Refusal> {
        let leaves = self.leaves_under(destination);
        if leaves.is_empty() {
            return Err(Refusal::Cleanup);
        }
        for index in leaves {
            let suffix =
                &self.cleanup.leaves[index].place.projections[destination.projections.len()..];
            let source = match (value, suffix) {
                (Owned::Leaf(local), []) => *local,
                (Owned::Record(fields), [field]) => fields
                    .iter()
                    .find(|(id, _)| id == field)
                    .map(|(_, local)| *local)
                    .ok_or(Refusal::Cleanup)?,
                _ => return Err(Refusal::Cleanup),
            };
            let (carrier, flag) = (
                self.cleanup.leaves[index].carrier,
                self.cleanup.leaves[index].flag,
            );
            self.op(I::LocalGet(source));
            self.op(I::LocalSet(carrier));
            self.set(flag, 1);
        }
        Ok(())
    }

    fn clear(&mut self, place: &CleanupPlace) -> Result<(), Refusal> {
        let leaves = self.leaves_under(place);
        if leaves.is_empty() {
            return Err(Refusal::Cleanup);
        }
        for index in leaves {
            let flag = self.cleanup.leaves[index].flag;
            self.set(flag, 0);
        }
        Ok(())
    }

    /// Move liveness and carriers leaf by leaf. A renewed handle produced at
    /// this expression replaces the source carrier of a whole-value move.
    fn transfer(
        &mut self,
        source: &CleanupPlace,
        destination: &CleanupPlace,
        value: &Owned,
    ) -> Result<(), Refusal> {
        let sources = self.leaves_under(source);
        if sources.is_empty() {
            return Err(Refusal::Cleanup);
        }
        for from in sources {
            let suffix =
                self.cleanup.leaves[from].place.projections[source.projections.len()..].to_vec();
            let mut target = destination.clone();
            target.projections.extend(suffix.iter().cloned());
            let to = self
                .cleanup
                .leaves
                .iter()
                .position(|leaf| leaf.place == target)
                .ok_or(Refusal::Cleanup)?;
            if self.cleanup.leaves[from].lifecycle != self.cleanup.leaves[to].lifecycle {
                return Err(Refusal::Cleanup);
            }
            let (from_carrier, from_flag) = (
                self.cleanup.leaves[from].carrier,
                self.cleanup.leaves[from].flag,
            );
            let (to_carrier, to_flag) = (
                self.cleanup.leaves[to].carrier,
                self.cleanup.leaves[to].flag,
            );
            // Only a selected Vec handle is renewed by the host on transfer.
            match (value, suffix.as_slice(), self.cleanup.leaves[to].lifecycle) {
                (Owned::Leaf(renewed), [], Lifecycle::Vector) => self.op(I::LocalGet(*renewed)),
                _ => self.op(I::LocalGet(from_carrier)),
            }
            self.op(I::LocalSet(to_carrier));
            self.op(I::LocalGet(from_flag));
            self.op(I::LocalSet(to_flag));
            self.set(from_flag, 0);
        }
        Ok(())
    }

    /// Replay, in plan vector order, every transition the plan places at `at`.
    pub(super) fn post(&mut self, at: &ExpressionId, value: Owned) -> Result<(), Refusal> {
        let plan = self.cleanup.plan;
        for (block_index, block) in plan.blocks.iter().enumerate() {
            for (index, transition) in block.transitions.iter().enumerate() {
                let hit = match transition {
                    Tr::Initialize {
                        at: point,
                        destination,
                    } if point == at => {
                        self.assign(destination, &value)?;
                        true
                    }
                    Tr::Renew {
                        at: point,
                        source,
                        destination,
                    } if point == at => {
                        self.clear(source)?;
                        self.assign(destination, &value)?;
                        true
                    }
                    Tr::Transfer {
                        at: point,
                        source,
                        destination,
                    } if point == at => {
                        self.transfer(source, destination, &value)?;
                        true
                    }
                    Tr::ReserveRenewal { at: point, binding } if point == at => {
                        // A reserved cleanup position: no runtime liveness change,
                        // but its binding must be a modelled leaf.
                        if self.leaves_under(binding).is_empty() {
                            return Err(Refusal::Cleanup);
                        }
                        true
                    }
                    Tr::CallCommit { call, arguments } if call == at => {
                        for argument in arguments {
                            self.clear(&argument.source)?;
                        }
                        true
                    }
                    _ => false,
                };
                if hit {
                    self.cleanup.applied[block_index][index] = true;
                }
            }
        }
        Ok(())
    }

    /// Emit one exit's finalizers in exact plan order, each guarded by the
    /// plan's own flag.
    fn finalize(&mut self, exit: usize) -> Result<(), Refusal> {
        let plan = self.cleanup.plan;
        let actions: &[FinalizeAction] = &plan.exits[exit].finalize_in_order;
        for action in actions {
            if action.active_case.is_some() {
                return Err(Refusal::Cleanup);
            }
            let leaf = self
                .cleanup
                .leaves
                .iter()
                .find(|leaf| leaf.place == action.source && leaf.flag_id == action.guard_flag)
                .ok_or(Refusal::Cleanup)?;
            let (lifecycle, carrier, flag) = (leaf.lifecycle, leaf.carrier, leaf.flag);
            let (expected, function) = match lifecycle {
                Lifecycle::Vector => (VEC_DROP_LIFECYCLE_ID, import::VEC_DROP),
                Lifecycle::Text => (STRING_DROP_LIFECYCLE_ID, import::STRING_DROP),
                Lifecycle::Payload => (BYTES_DROP_LIFECYCLE_ID, import::OWNER_DROP),
            };
            if action.lifecycle_id.as_str() != expected {
                return Err(Refusal::Cleanup);
            }
            self.op(I::LocalGet(flag));
            self.op(I::If(BlockType::Empty));
            self.op(I::LocalGet(self.domain));
            self.op(I::LocalGet(carrier));
            self.checked(function);
            self.set(flag, 0);
            self.op(I::I64Const(0));
            self.op(I::LocalSet(carrier));
            self.op(I::End);
        }
        self.cleanup.emitted[exit] = true;
        Ok(())
    }

    /// Normal end of one nested lexical region.
    fn scope_end(&mut self, region: CleanupRegionId) -> Result<(), Refusal> {
        let plan = self.cleanup.plan;
        let mut exits = plan.exits.iter().enumerate().filter(|(_, exit)| {
            matches!(exit.continuation, ExitContinuation::Continue(_))
                && exit.leaves_regions.as_slice() == [region]
        });
        let (index, _) = exits.next().ok_or(Refusal::Cleanup)?;
        if exits.next().is_some() {
            return Err(Refusal::Cleanup);
        }
        self.finalize(index)
    }

    /// End of a lowered nested block: find the unique non-root region whose
    /// every slot is storage introduced directly by this block, if any.
    pub(super) fn block_end(&mut self, block: &E) -> Result<(), Refusal> {
        let K::Block { statements, tail } = &block.kind else {
            return Err(unsupported("scope"));
        };
        let mut direct = Vec::new();
        let mut pending: Vec<&E> = Vec::new();
        for statement in statements {
            if let crate::hir::ResolvedStatement::Let { binding, .. } = statement {
                direct.push(StorageId::Value(binding.id.clone()));
            }
            for child in 0..statement.child_count() {
                pending.extend(statement.child(child));
            }
        }
        pending.push(tail);
        while let Some(expression) = pending.pop() {
            if matches!(expression.kind, K::Block { .. }) {
                continue;
            }
            direct.push(StorageId::Temporary(expression.id.clone()));
            pending.extend(crate::hir::lexical_exit::ChildCursor::new(expression));
        }
        let plan = self.cleanup.plan;
        let mut regions = plan.regions.iter().filter(|region| {
            region.parent.is_some()
                && !region.slots.is_empty()
                && region.slots.iter().all(|slot| match slot {
                    StorageId::CallArgument { call, .. } => {
                        direct.contains(&StorageId::Temporary(call.clone()))
                    }
                    other => direct.contains(other),
                })
        });
        let Some(region) = regions.next() else {
            return Ok(());
        };
        if regions.next().is_some() {
            return Err(Refusal::Cleanup);
        }
        self.scope_end(region.id)
    }

    /// Successful completion: the scalar result is already staged; there are
    /// no postconditions in this profile, so the commit exit's non-result
    /// cleanup runs next and the result is published only after it. `body`
    /// is the function body block the canonical scalar result source names.
    pub(super) fn commit(&mut self, body: &ExpressionId) -> Result<(), Refusal> {
        let plan = self.cleanup.plan;
        let mut commits =
            plan.exits.iter().enumerate().filter(|(_, exit)| {
                matches!(exit.continuation, ExitContinuation::CommitResult { .. })
            });
        let (index, exit) = commits.next().ok_or(Refusal::Cleanup)?;
        if commits.next().is_some()
            || !matches!(&exit.continuation,
                ExitContinuation::CommitResult { source: CleanupResultSource::Scalar { expression } }
                    if expression == body)
        {
            return Err(Refusal::Cleanup);
        }
        self.finalize(index)
    }

    /// The exact failure exit for `expression`: select the status first (the
    /// host keeps the first selection sticky), end exactly the leased views
    /// whose loans are live at `expression`, bracket an unfinished
    /// construction with abort/settle, then emit the exit's finalizers
    /// verbatim and return the selected failure.
    pub(super) fn fail(
        &mut self,
        expression: &ExpressionId,
        failure: Failure,
        status: i32,
    ) -> Result<(), Refusal> {
        let plan = self.cleanup.plan;
        let source = StatusSourceId {
            expression: expression.clone(),
            lane: StatusLane::OperationFailure,
        };
        let producer = &plan
            .status_sources
            .iter()
            .find(|candidate| candidate.id == source)
            .ok_or(Refusal::Cleanup)?
            .producer;
        let agrees = match (failure, producer) {
            (Failure::VecAllocation, StatusProducer::PropagatedCall { callee }) => {
                callee.as_str() == crate::vec_ops::WITH_CAPACITY_ID
            }
            (Failure::VecPush, StatusProducer::PropagatedCall { callee }) => {
                callee.as_str() == crate::vec_ops::PUSH_ID
            }
            (Failure::VecRead, StatusProducer::PropagatedCall { callee }) => {
                callee.as_str() == crate::vec_field::ID
            }
            (Failure::Conversion, StatusProducer::PropagatedCall { callee }) => {
                callee.as_str() == crate::string_ops::I64_FROM_USIZE_ID
            }
            (
                Failure::Arithmetic(expected),
                StatusProducer::CheckedArithmetic { operation, .. },
            ) => *operation == expected,
            _ => false,
        };
        if !agrees {
            return Err(Refusal::Cleanup);
        }
        let mut exits = plan.exits.iter().enumerate().filter(|(_, exit)| {
            matches!(&exit.continuation, ExitContinuation::ReturnFailure { source: selected }
                if selected == &source)
        });
        let (index, _) = exits.next().ok_or(Refusal::Cleanup)?;
        if exits.next().is_some() {
            return Err(Refusal::Cleanup);
        }
        for (block_index, block) in plan.blocks.iter().enumerate() {
            for (transition_index, transition) in block.transitions.iter().enumerate() {
                if matches!(transition, Tr::SelectFailure { source: selected } if selected == &source)
                {
                    self.cleanup.applied[block_index][transition_index] = true;
                }
            }
        }
        let live = self.live_views_at(expression)?;
        self.op(I::I32Const(status));
        self.op(I::Call(import::SELECT_FAILURE));
        // Only the loans the plan holds live here end, and they end before any
        // owner finalizer. Normal-path scopes are untouched: this code exits.
        for view in live {
            self.end_view(view);
        }
        let building = self.phase == super::lower::Phase::Building;
        if building {
            self.op(I::LocalGet(self.domain));
            self.checked(import::ABORT);
        }
        self.finalize(index)?;
        if building {
            self.op(I::LocalGet(self.domain));
            self.checked(import::SETTLE_ABORT);
        }
        self.op(I::Call(import::RETURN_FAILURE));
        self.op(I::Unreachable);
        Ok(())
    }

    /// Every transition and every exit with observable work was reproduced.
    pub(super) fn census(&self) -> Result<(), Refusal> {
        let plan = self.cleanup.plan;
        if self
            .cleanup
            .applied
            .iter()
            .flatten()
            .any(|applied| !applied)
        {
            return Err(Refusal::Cleanup);
        }
        for (exit, emitted) in plan.exits.iter().zip(&self.cleanup.emitted) {
            let required = match &exit.continuation {
                ExitContinuation::CommitResult { .. } | ExitContinuation::ReturnFailure { .. } => {
                    true
                }
                ExitContinuation::Continue(_) => !exit.finalize_in_order.is_empty(),
                _ => return Err(Refusal::Cleanup),
            };
            if required && !emitted {
                return Err(Refusal::Cleanup);
            }
        }
        Ok(())
    }
}
