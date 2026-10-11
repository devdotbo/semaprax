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

struct Keyed<'p> {
    key: &'p ExpressionId,
    block: u32,
    transitions: Vec<&'p CleanupTransition>,
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
                    | CleanupTransition::Renew { at, .. } => at,
                    CleanupTransition::CallCommit { call, .. } => call,
                    _ => return Err(refuse("a transition kind outside the selected domain")),
                };
                match keyed.iter_mut().find(|entry| entry.key == key) {
                    Some(entry) if entry.block == block.id.0 => entry.transitions.push(transition),
                    Some(_) => return Err(refuse("one expression's transitions span blocks")),
                    None => keyed.push(Keyed {
                        key,
                        block: block.id.0,
                        transitions: vec![transition],
                    }),
                }
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
    pub(super) fn transitions(&self, key: &ExpressionId) -> Option<&[&'p CleanupTransition]> {
        self.keyed
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| entry.transitions.as_slice())
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
