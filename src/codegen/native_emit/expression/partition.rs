//! Preparation failure preserves the staged owner and its canonical exit.
use super::*;
impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn emit_partition_call(
        &mut self,
        expression: &ExpressionId,
        arguments: &[CValue],
        temporary: &str,
    ) -> Result<(), Diagnostic> {
        if self.output_profile
            != crate::codegen::native_emit::NativeOutputProfile::SelectedPartition
        {
            return Err(backend_error(
                "byte partition requires its checked private receiver",
            ));
        }
        let [owner, count, width] = arguments else {
            return Err(backend_error("native partition operand census"));
        };
        let plan = self
            .bytes_plan
            .ok_or_else(|| backend_error("native partition cleanup plan absent"))?;
        let (source, flag, _) = plan.call_argument(expression, 0)?;
        if source != owner.code {
            return Err(backend_error(
                "native partition owner is not its staged canonical argument",
            ));
        }
        let ticket = format!("spx_partition_ticket_{}", self.next_local);
        self.next_local += 1;
        self.line(&format!("spx_shb_partition_plan {ticket} = {{0}};"));
        self.line(&format!("spx_status = spx_shb_status(spx_ctx, spx_shb_partition_prepare(spx_shb_held(spx_ctx), &{source}, {}, {}, &{ticket}));", count.code, width.code));
        self.emit_operation_failure_check_at(expression)?;
        // The runtime publishes the complete child inventory only after all
        // checks and reservations. It clears this same staged source carrier.
        self.line(&format!(
            "spx_shb_partition_commit(spx_shb_held(spx_ctx), &{ticket}, &{source}, &{temporary});"
        ));
        self.line(&format!("{flag} = false;"));
        Ok(())
    }
}
