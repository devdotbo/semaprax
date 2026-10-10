//! Proof-only cleanup contours for backend execution guards.
//!
//! These certificates select no status, admit no owner or ABI, and cannot make
//! a fatal allocator route recoverable. Independent replay reconstructs them;
//! consumers use their finalizer vectors verbatim after selecting a guard failure.
use super::{BlockId, CleanupPlan, CleanupRegionId, FinalizeAction};
use crate::hir::ExpressionId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionBoundaryKind {
    FunctionEntry,
    LoopIteration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExecutionBoundaryCertificate {
    pub kind: ExecutionBoundaryKind,
    pub site: ExpressionId,
    pub block: BlockId,
    pub region: CleanupRegionId,
    pub leaves_regions: Vec<CleanupRegionId>,
    pub finalize_in_order: Vec<FinalizeAction>,
}

impl CleanupPlan {
    /// Exact invocation-local selection after ordinary HIR and replay checks.
    /// Missing, duplicate, stale-domain, or noncanonical index metadata refuses.
    pub fn execution_boundary(
        &self,
        kind: ExecutionBoundaryKind,
        site: &ExpressionId,
        resume: Option<BlockId>,
    ) -> Option<&ExecutionBoundaryCertificate> {
        let control = self.lexical_control.as_deref()?;
        if !matches!(
            self.schema,
            super::CLEANUP_PLAN_SCHEMA_V18 | super::CLEANUP_PLAN_SCHEMA_V19
        ) || control.schema != crate::hir::lexical_exit::LEXICAL_CONTROL_FLOW_SCHEMA_V1
            || resume.is_some_and(|entry| self.lexical_resume(entry).is_none())
        {
            return None;
        }
        let mut matches = control.execution_boundaries.iter().filter(|item| {
            item.kind == kind
                && &item.site == site
                && self.lexical_block_in_context(resume, item.block)
        });
        let item = matches.next()?;
        if matches.next().is_some()
            || self.blocks.get(item.block.0 as usize)?.id != item.block
            || self.regions.get(item.region.0 as usize)?.id != item.region
            || self.blocks.get(item.block.0 as usize)?.region != item.region
        {
            return None;
        }
        Some(item)
    }
}
