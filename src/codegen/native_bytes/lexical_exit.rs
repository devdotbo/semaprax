//! Exact per-site lexical finalizers; never fold these into terminal ordering.
use super::*;
impl NativeBytesPlan {
    pub(super) fn lexical_finalizers(
        &self,
        plan: &crate::cleanup_plan::CleanupPlan,
        site: &ExpressionId,
    ) -> Result<String, Diagnostic> {
        let (_, exit) = plan
            .lexical_exit(site)
            .ok_or_else(|| error("native lexical exit is not authenticated"))?;
        self.exit_finalizers(exit, false)
    }

    pub(in crate::codegen) fn exit_finalizers(
        &self,
        exit: &crate::cleanup_plan::ExitTarget,
        terminal: bool,
    ) -> Result<String, Diagnostic> {
        let mut output = String::new();
        for action in &exit.finalize_in_order {
            let slot = self
                .slots
                .get(&action.source)
                .ok_or_else(|| error("native lexical finalizer has no physical slot"))?;
            if slot.flag != format!("spx_bytes_live_{}", action.guard_flag.0) {
                return Err(error("native lexical finalizer flag differs"));
            }
            output.push_str(&self.emit_finalizers(std::slice::from_ref(slot), terminal));
        }
        Ok(output)
    }
}
