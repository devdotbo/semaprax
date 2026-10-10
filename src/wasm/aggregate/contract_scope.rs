//! Complete the exact successful contract child scope in its selected domain.
use super::*;

impl Emitter<'_> {
    pub(super) fn emit_contract_scope_success(
        &mut self,
        at: &ExpressionId,
    ) -> Result<(), Diagnostic> {
        if self.cleanup_plan.lexical_control.is_none() {
            return Ok(());
        }
        let plan = self.cleanup_plan;
        let mut edges = plan.edges.iter().filter(|edge| {
            plan.lexical_block_in_context(self.lexical_resume, edge.from)
                && matches!(&edge.condition, crate::cleanup_plan::EdgeCondition::BooleanResult(id, true) if id == at)
        });
        let edge = edges
            .next()
            .ok_or_else(|| error("lexical contract lacks its successful edge"))?;
        if edges.next().is_some() {
            return Err(error("lexical contract repeats its successful edge"));
        }
        let block = plan
            .blocks
            .get(edge.to.0 as usize)
            .filter(|block| block.id == edge.to)
            .ok_or_else(|| error("lexical contract successful block is absent"))?;
        let mut exits = plan.exits.iter().filter(|exit| {
            exit.from == block.id
                && exit.leaves_regions.as_slice() == [block.region]
                && matches!(
                    exit.continuation,
                    crate::cleanup_plan::ExitContinuation::Continue(_)
                )
        });
        let exit = exits
            .next()
            .ok_or_else(|| error("lexical contract lacks successful scope completion"))?;
        if exits.next().is_some() {
            return Err(error(
                "lexical contract repeats successful scope completion",
            ));
        }
        self.emit_cleanup_actions(&exit.finalize_in_order)
    }
}
