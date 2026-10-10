//! Capture actual builder state at an execution guard; no new language status lane.
use super::*;
use crate::cleanup_plan::{ExecutionBoundaryCertificate, ExecutionBoundaryKind};
impl PlanBuilder<'_> {
    pub(super) fn record_execution_boundary(
        &mut self,
        kind: ExecutionBoundaryKind,
        site: &ExpressionId,
        block: BlockId,
        state: &FlowState,
        region: CleanupRegionId,
    ) -> Result<(), Diagnostic> {
        if self.lexical.is_none() {
            return Ok(());
        }
        if self
            .blocks
            .get(block.0 as usize)
            .is_none_or(|item| item.id != block || item.region != region)
            || self
                .regions
                .get(region.0 as usize)
                .is_none_or(|item| item.id != region)
        {
            return Err(plan_error(
                "execution boundary has noncanonical block or region",
            ));
        }
        let finalize_in_order = self.finalizers_for(state, |_| true);
        let leaves_regions = self.region_chain(region);
        let control = self.lexical.as_mut().unwrap();
        if control
            .certificate
            .execution_boundaries
            .iter()
            .any(|item| item.block == block && item.kind == kind && &item.site == site)
        {
            return Err(plan_error("execution boundary is duplicated"));
        }
        control
            .certificate
            .execution_boundaries
            .push(ExecutionBoundaryCertificate {
                kind,
                site: site.clone(),
                block,
                region,
                leaves_regions,
                finalize_in_order,
            });
        Ok(())
    }
}
