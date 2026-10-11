//! Canonical cleanup-plan proof for the selected function.
//!
//! The attached `cleanup_plan` is the owner of drop, loan-settlement and
//! staging order. This module reads its vectors verbatim: transitions in block
//! order keyed by their exact expression, and each exit's `finalize_in_order`
//! unchanged. Lookups only locate entries; nothing is sorted, merged, unioned
//! or repaired. Any shape this proof cannot bind exactly is refused:
//! variant inventories, live parameters, lexical exits or resume bodies,
//! non-operation status lanes, failure paths with transitions other than
//! their own status selection, ambiguous edges, and owned results.
//!
//! One expression's transitions occupy one block, or two phases joined by
//! that same expression's own operation-failure split: the block holding the
//! pre-status phase (a non-deferred `CallCommit`, a `ReserveRenewal`) ends in
//! exactly `[StatusZero(source), StatusNonzero(source)]` for the source at
//! this expression, and the post-status phase (deferred commit, result
//! `Initialize`, renewal or transfer) lies in that `StatusZero` target. Each
//! phase keeps its own vector order; the receiver applies the pre-status phase
//! before the status test and the post-status phase after it.
use super::error;
use crate::cleanup::{FieldLivenessShape, LivenessFlagId};
use crate::cleanup_plan::{
    CleanupPlace, CleanupPlan, CleanupResultSource, CleanupTerminator, CleanupTransition,
    EdgeCondition, ExitContinuation, ExitTarget, StatusLane, StatusSourceId, StorageId,
};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ExpressionId, ResolvedFunction};

/// One finalizable leaf, in canonical slot order then field order.
pub(super) struct Leaf<'p> {
    pub(super) place: CleanupPlace,
    pub(super) flag: LivenessFlagId,
    pub(super) lifecycle: &'p DeclarationId,
}

/// The transitions one plan block places at one expression, in vector order.
pub(super) struct Phase<'p> {
    block: u32,
    /// The block ends in this expression's own status split, so these
    /// transitions precede the status test.
    pub(super) pre_status: bool,
    pub(super) transitions: Vec<&'p CleanupTransition>,
}

struct Keyed<'p> {
    key: &'p ExpressionId,
    phases: Vec<Phase<'p>>,
}

pub(super) struct PlanProof<'p> {
    plan: &'p CleanupPlan,
    leaves: Vec<Leaf<'p>>,
    keyed: Vec<Keyed<'p>>,
    failures: Vec<(&'p StatusSourceId, &'p ExitTarget)>,
    success: &'p ExitTarget,
}

fn refuse(message: &str) -> Diagnostic {
    error(&format!("cleanup order cannot be proved: {message}"))
}

impl<'p> PlanProof<'p> {
    pub(super) fn admit(function: &'p ResolvedFunction) -> Result<Self, Diagnostic> {
        let plan = &function.cleanup_plan;
        if !plan.entry_state.live_owned_parameters.is_empty()
            || !plan.entry_state.conditional_owned_parameters.is_empty()
        {
            return Err(refuse("the selected function has live owned parameters"));
        }
        if let Some(control) = plan.lexical_control.as_deref() {
            if !control.exits.is_empty() || !control.resumes.is_empty() {
                return Err(refuse("lexical exits or resume bodies are present"));
            }
            if control
                .execution_boundaries
                .iter()
                .any(|boundary| !boundary.finalize_in_order.is_empty())
            {
                return Err(refuse("an execution boundary finalizes a live owner"));
            }
        }
        let leaves = flatten(plan)?;
        let mut keyed: Vec<Keyed<'p>> = Vec::new();
        for block in &plan.blocks {
            for transition in &block.transitions {
                let key = match transition {
                    CleanupTransition::SelectFailure { .. } => continue,
                    CleanupTransition::Initialize { at, .. }
                    | CleanupTransition::Transfer { at, .. }
                    | CleanupTransition::Renew { at, .. }
                    | CleanupTransition::ReserveRenewal { at, .. } => at,
                    CleanupTransition::CallCommit { call, .. } => call,
                    _ => return Err(refuse("a transition kind outside the selected domain")),
                };
                let id = block.id.0;
                match keyed.iter_mut().find(|entry| entry.key == key) {
                    Some(entry) => {
                        let single = entry.phases.len() == 1;
                        let Some(last) = entry.phases.last_mut() else {
                            return Err(refuse("a keyed expression has no phase"));
                        };
                        if last.block == id {
                            last.transitions.push(transition);
                        } else if single
                            && last.pre_status
                            && status_success(plan, last.block, key) == Some(id)
                        {
                            entry.phases.push(Phase {
                                block: id,
                                pre_status: false,
                                transitions: vec![transition],
                            });
                        } else {
                            return Err(refuse("one expression's transitions span blocks"));
                        }
                    }
                    None => keyed.push(Keyed {
                        key,
                        phases: vec![Phase {
                            block: id,
                            pre_status: status_success(plan, id, key).is_some(),
                            transitions: vec![transition],
                        }],
                    }),
                }
            }
        }
        for entry in &keyed {
            // A post-status phase exists only beyond its own split, and the
            // pre-status phase of a split key carries no result or transfer.
            if entry.phases.iter().any(|phase| {
                phase.pre_status
                    && phase.transitions.iter().any(|transition| {
                        !matches!(
                            transition,
                            CleanupTransition::CallCommit { .. }
                                | CleanupTransition::ReserveRenewal { .. }
                        )
                    })
            }) {
                return Err(refuse(
                    "an owner is produced before its own status is known",
                ));
            }
        }
        let mut failures = Vec::with_capacity(plan.status_sources.len());
        for source in &plan.status_sources {
            if source.id.lane != StatusLane::OperationFailure {
                return Err(refuse("a status lane other than operation failure"));
            }
            failures.push((&source.id, failure_exit(plan, &source.id)?));
        }
        let mut commits = plan.exits.iter().filter(|exit| {
            matches!(exit.continuation, ExitContinuation::CommitResult { .. })
                && plan.lexical_block_in_context(None, exit.from)
        });
        let success = commits
            .next()
            .ok_or_else(|| refuse("no result commit exit"))?;
        if commits.next().is_some() {
            return Err(refuse("ambiguous result commit exit"));
        }
        if !matches!(
            success.continuation,
            ExitContinuation::CommitResult {
                source: CleanupResultSource::Scalar { .. }
            }
        ) {
            return Err(refuse("the selected result is not a scalar commit"));
        }
        for exit in &plan.exits {
            if exit
                .finalize_in_order
                .iter()
                .any(|action| action.active_case.is_some())
            {
                return Err(refuse("a conditional variant finalizer"));
            }
        }
        Ok(Self {
            plan,
            leaves,
            keyed,
            failures,
            success,
        })
    }
    pub(super) fn plan(&self) -> &'p CleanupPlan {
        self.plan
    }
    pub(super) fn leaves(&self) -> &[Leaf<'p>] {
        &self.leaves
    }
    pub(super) fn leaf(&self, flag: LivenessFlagId) -> Option<&Leaf<'p>> {
        self.leaves.iter().find(|leaf| leaf.flag == flag)
    }
    /// Leaves under `prefix`, in canonical order.
    pub(super) fn leaves_under(&self, prefix: &CleanupPlace) -> Vec<&Leaf<'p>> {
        self.leaves
            .iter()
            .filter(|leaf| {
                leaf.place.storage == prefix.storage
                    && leaf.place.projections.starts_with(&prefix.projections)
            })
            .collect()
    }
    /// The phases keyed by `key`, in plan block order.
    pub(super) fn phases(&self, key: &ExpressionId) -> Option<&[Phase<'p>]> {
        self.keyed
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.phases.as_slice())
    }
    pub(super) fn keys(&self) -> impl Iterator<Item = &'p ExpressionId> + '_ {
        self.keyed.iter().map(|entry| entry.key)
    }
    pub(super) fn failure(&self, expression: &ExpressionId) -> Option<&'p ExitTarget> {
        self.failures
            .iter()
            .find(|(source, _)| &source.expression == expression)
            .map(|(_, exit)| *exit)
    }
    pub(super) fn failures(&self) -> impl Iterator<Item = &'p ExitTarget> + '_ {
        self.failures.iter().map(|(_, exit)| *exit)
    }
    pub(super) fn success(&self) -> &'p ExitTarget {
        self.success
    }
    /// The normal scope end of the unique region holding every anchor.
    pub(super) fn region_exit(
        &self,
        anchors: &[StorageId],
    ) -> Result<Option<&'p ExitTarget>, Diagnostic> {
        if anchors.is_empty() {
            return Ok(None);
        }
        let mut regions = self
            .plan
            .regions
            .iter()
            .filter(|region| anchors.iter().all(|anchor| region.slots.contains(anchor)));
        let region = regions
            .next()
            .ok_or_else(|| refuse("a block's owners are not in one region"))?;
        if regions.next().is_some() {
            return Err(refuse("a block's owners match several regions"));
        }
        let exit = self
            .plan
            .exits
            .get(region.normal_scope_end.0 as usize)
            .filter(|exit| exit.id == region.normal_scope_end)
            .ok_or_else(|| refuse("a region's normal scope end is not indexed"))?;
        Ok(Some(exit))
    }
}

/// The `StatusZero` target of `block` when it ends in exactly the operation
/// split of the status source at `key`: `[StatusZero, StatusNonzero]` edges
/// indexed by their own ids, leaving `block`, naming one source.
fn status_success(plan: &CleanupPlan, block: u32, key: &ExpressionId) -> Option<u32> {
    let current = plan
        .blocks
        .get(block as usize)
        .filter(|candidate| candidate.id.0 == block)?;
    let CleanupTerminator::Branch(edges) = &current.terminator else {
        return None;
    };
    let [success, failure] = edges.as_slice() else {
        return None;
    };
    let edge = |id: &crate::cleanup_plan::EdgeId| {
        plan.edges
            .get(id.0 as usize)
            .filter(|edge| edge.id == *id && edge.from == current.id)
    };
    let (success, failure) = (edge(success)?, edge(failure)?);
    match (&success.condition, &failure.condition) {
        (EdgeCondition::StatusZero(zero), EdgeCondition::StatusNonzero(nonzero))
            if zero == nonzero
                && &zero.expression == key
                && zero.lane == StatusLane::OperationFailure
                && plan.lexical_block_in_context(None, current.id) =>
        {
            Some(success.to.0)
        }
        _ => None,
    }
}

/// The exact `StatusNonzero` edge, through blocks whose only transition is
/// this source's own selection, to its `ReturnFailure` exit.
fn failure_exit<'p>(
    plan: &'p CleanupPlan,
    source: &StatusSourceId,
) -> Result<&'p ExitTarget, Diagnostic> {
    let mut edges = plan.edges.iter().filter(|edge| {
        matches!(&edge.condition, EdgeCondition::StatusNonzero(selected) if selected == source)
            && plan.lexical_block_in_context(None, edge.from)
    });
    let edge = edges
        .next()
        .ok_or_else(|| refuse("a status source has no failure edge"))?;
    if edges.next().is_some() {
        return Err(refuse("a status source has several failure edges"));
    }
    let mut block = edge.to;
    let mut selected = 0usize;
    for _ in 0..=plan.blocks.len() {
        let current = plan
            .blocks
            .get(block.0 as usize)
            .filter(|candidate| candidate.id == block)
            .ok_or_else(|| refuse("a failure block is not indexed"))?;
        for transition in &current.transitions {
            match transition {
                CleanupTransition::SelectFailure { source: chosen } if chosen == source => {
                    selected += 1;
                }
                _ => return Err(refuse("a failure path transfers or initializes an owner")),
            }
        }
        match &current.terminator {
            CleanupTerminator::Exit(id) => {
                let exit = plan
                    .exits
                    .get(id.0 as usize)
                    .filter(|exit| exit.id == *id)
                    .ok_or_else(|| refuse("a failure exit is not indexed"))?;
                if selected != 1
                    || !matches!(&exit.continuation, ExitContinuation::ReturnFailure { source: chosen } if chosen == source)
                {
                    return Err(refuse(
                        "a failure path does not select exactly its own status",
                    ));
                }
                return Ok(exit);
            }
            CleanupTerminator::Goto(id) => {
                let next = plan
                    .edges
                    .get(id.0 as usize)
                    .filter(|edge| edge.id == *id && edge.condition == EdgeCondition::Always)
                    .ok_or_else(|| refuse("a failure path branches"))?;
                block = next.to;
            }
            CleanupTerminator::Branch(_) => return Err(refuse("a failure path branches")),
        }
    }
    Err(refuse("a failure path does not terminate"))
}

/// Flatten every slot's liveness shape into leaves without recursion.
fn flatten(plan: &CleanupPlan) -> Result<Vec<Leaf<'_>>, Diagnostic> {
    let mut leaves: Vec<Leaf<'_>> = Vec::new();
    for slot in &plan.slots {
        let mut pending: Vec<(Vec<DeclarationId>, &FieldLivenessShape)> =
            vec![(Vec::new(), &slot.field_liveness_shape)];
        while let Some((projections, shape)) = pending.pop() {
            match shape {
                FieldLivenessShape::NoDrop => {}
                FieldLivenessShape::Leaf { flag, lifecycle } => {
                    if leaves.iter().any(|leaf| leaf.flag == *flag) {
                        return Err(refuse("a liveness flag repeats"));
                    }
                    leaves.push(Leaf {
                        place: CleanupPlace {
                            storage: slot.storage.clone(),
                            projections,
                        },
                        flag: *flag,
                        lifecycle,
                    });
                }
                FieldLivenessShape::Record { fields, .. } => {
                    // Reverse push keeps declaration order on pop.
                    for field in fields.iter().rev() {
                        if projections.len() >= 8 {
                            return Err(refuse("a record cleanup shape is too deep"));
                        }
                        let mut path = projections.clone();
                        path.push(field.field.clone());
                        pending.push((path, &field.shape));
                    }
                }
                FieldLivenessShape::Variant { .. } => {
                    return Err(refuse("a conditional variant cleanup shape"));
                }
            }
        }
    }
    Ok(leaves)
}
