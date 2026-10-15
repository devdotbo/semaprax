//! Completed ordinary String clones may settle at a statement-shaped exit.
//! This never grants an exit across expression evaluation: independent source/HIR
//! contour validation must first reject initializer, argument and return-operand
//! crossings. The builder reaches this predicate only after ordinary evaluation.
use super::*;
impl PlanBuilder<'_> {
    pub(super) fn completed_lexical_storage(
        &self,
        storage: &StorageId,
    ) -> Result<bool, Diagnostic> {
        let StorageId::Temporary(expression) = storage else {
            return Ok(matches!(storage, StorageId::Value(_)));
        };
        let Some(slot) = self
            .storage_to_slot
            .get(storage)
            .and_then(|id| self.slots.get(id.0 as usize))
        else {
            return Ok(false);
        };
        if slot.storage != *storage || slot.ty != ResolvedType::String {
            return Ok(false);
        }
        // Re-resolve the exact current body expression; inventory metadata or a
        // temporary-shaped ID alone cannot authorize a physical String drop.
        let expression = lexical_control::expression_at(&self.function.body, expression)?;
        Ok(self.owns_string_clone(expression))
    }
    pub(super) fn require_completed_lexical_finalizers(
        &self,
        actions: &[FinalizeAction],
    ) -> Result<(), Diagnostic> {
        for action in actions {
            if !self.completed_lexical_storage(&action.source.storage)? {
                return Err(plan_error("lexical success contour contains live unstaged temporary or call argument storage"));
            }
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests;
