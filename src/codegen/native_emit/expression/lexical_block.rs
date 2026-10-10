//! Keep v18 variant Block transfers on their certified side of scope cleanup.
use super::*;

impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn lexical_variant_block(&self, block: &ResolvedExpr) -> Result<bool, Diagnostic> {
        Ok(
            self.function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18
                && block.ownership == hir::OwnershipMode::Own
                && variant_declaration_id(self.program, &block.ty)?.is_some(),
        )
    }

    pub(super) fn emit_lexical_variant_block_result(
        &mut self,
        block: &ResolvedExpr,
        value: &CValue,
    ) -> Result<(), Diagnostic> {
        let layout = self.variant_layout(&block.ty)?;
        let phase = self
            .bytes_plan
            .ok_or_else(|| backend_error("native lexical variant Block has no producer plan"))?
            .apply_variant_into_at(
                &block.id,
                &value.code,
                &layout,
                &crate::cleanup_plan::StorageId::Temporary(block.id.clone()),
            )?;
        for line in phase.lines() {
            self.line(line);
        }
        Ok(())
    }
}
