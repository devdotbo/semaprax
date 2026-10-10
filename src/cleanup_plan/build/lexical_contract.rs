//! Reuse exact authored postcondition scopes across disjoint return domains.
//! Source storage identities stay unique; scope endpoints remain context-selected.
use super::*;
impl PlanBuilder<'_> {
    pub(super) fn lower_contract_expression(
        &mut self,
        contract: &ResolvedExpr,
        block: BlockId,
        state: FlowState,
        parent: CleanupRegionId,
        phase: ContractPhase,
        ordinal: usize,
    ) -> Result<(BlockId, FlowState), Diagnostic> {
        let scoped = self.begin_ensure_regions(phase, ordinal)?;
        let lowered = (|| {
            let region = self.new_region(parent)?;
            let entry = self.new_block(region)?;
            let edge = self.new_edge(block, entry, EdgeCondition::Always)?;
            self.terminate(block, CleanupTerminator::Goto(edge))?;
            let evaluated = self.lower_expr(contract, entry, state, region)?;
            let (success, state) = self.lower_contract(
                contract,
                evaluated.block,
                evaluated.state,
                region,
                phase,
                ordinal,
            )?;
            self.exit_scope(success, state, region)
        })();
        let completed = self.finish_ensure_regions(scoped);
        let result = lowered?;
        completed?;
        Ok(result)
    }

    fn begin_ensure_regions(
        &mut self,
        phase: ContractPhase,
        ordinal: usize,
    ) -> Result<bool, Diagnostic> {
        let Some(control) = &mut self.lexical else {
            return Ok(false);
        };
        if phase != ContractPhase::Ensures {
            return Ok(false);
        }
        if control.current_ensure.is_some() {
            return Err(plan_error("postcondition scope template is already active"));
        }
        if ordinal == control.ensure_regions.len() {
            control.ensure_regions.push(Vec::new());
        } else if ordinal > control.ensure_regions.len() {
            return Err(plan_error("postcondition scope template skipped a clause"));
        }
        control.current_ensure = Some((ordinal, 0));
        Ok(true)
    }

    fn finish_ensure_regions(&mut self, scoped: bool) -> Result<(), Diagnostic> {
        if !scoped {
            return Ok(());
        }
        let control = self
            .lexical
            .as_mut()
            .ok_or_else(|| plan_error("postcondition template lost lexical context"))?;
        let (ordinal, count) = control
            .current_ensure
            .take()
            .ok_or_else(|| plan_error("postcondition scope template is not active"))?;
        if control.ensure_regions.get(ordinal).map(Vec::len) != Some(count) {
            return Err(plan_error(
                "postcondition scope template did not replay its exact authored shape",
            ));
        }
        Ok(())
    }

    pub(super) fn reuse_ensure_region(
        &mut self,
        parent: CleanupRegionId,
    ) -> Result<Option<CleanupRegionId>, Diagnostic> {
        let Some(control) = &mut self.lexical else {
            return Ok(None);
        };
        let Some((ordinal, index)) = control.current_ensure else {
            return Ok(None);
        };
        let retained = control
            .ensure_regions
            .get(ordinal)
            .ok_or_else(|| plan_error("postcondition scope template is missing"))?;
        let region = retained.get(index).copied();
        control.current_ensure = Some((
            ordinal,
            index
                .checked_add(1)
                .ok_or_else(|| plan_error("postcondition scope cursor overflow"))?,
        ));
        if let Some(region) = region {
            if self
                .regions
                .get(region.0 as usize)
                .is_none_or(|item| item.id != region || item.parent != Some(parent))
            {
                return Err(plan_error(
                    "postcondition scope template changed its exact parent contour",
                ));
            }
        } else if index != retained.len() {
            return Err(plan_error(
                "postcondition scope template skipped a retained region",
            ));
        }
        Ok(region)
    }

    pub(super) fn retain_ensure_region(
        &mut self,
        region: CleanupRegionId,
    ) -> Result<(), Diagnostic> {
        let Some(control) = &mut self.lexical else {
            return Ok(());
        };
        let Some((ordinal, next)) = control.current_ensure else {
            return Ok(());
        };
        let retained = control
            .ensure_regions
            .get_mut(ordinal)
            .ok_or_else(|| plan_error("postcondition scope template is missing"))?;
        if next == 0 || next - 1 != retained.len() {
            return Err(plan_error(
                "postcondition scope template appended a noncanonical region",
            ));
        }
        retained.push(region);
        Ok(())
    }

    pub(super) fn reuse_ensure_slot(
        &self,
        storage: &StorageId,
        ty: &ResolvedType,
        region: CleanupRegionId,
    ) -> Result<(), Diagnostic> {
        let shared = self
            .lexical
            .as_ref()
            .is_some_and(|control| control.current_ensure.is_some());
        let exact = self
            .storage_to_slot
            .get(storage)
            .and_then(|id| self.slots.get(id.0 as usize))
            .is_some_and(|slot| &slot.storage == storage && &slot.ty == ty)
            && self
                .regions
                .get(region.0 as usize)
                .is_some_and(|owner| owner.id == region && owner.slots.contains(storage));
        if shared && exact {
            Ok(())
        } else {
            Err(plan_error(format!(
                "cleanup storage `{storage:?}` is duplicated"
            )))
        }
    }
}
