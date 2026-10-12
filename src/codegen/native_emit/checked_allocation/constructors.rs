//! Constructor out cells remain dead until both semantic and physical success.
use super::*;
use crate::string_ops::StringOp;
impl<O: COutput> CEmitter<'_, O> {
    pub(in crate::codegen::native_emit) fn checked_string_constructor(
        &mut self, site: &ExpressionId, operation: AllocationOperation,
        call: &str, semantic_failure: bool,
    ) -> Result<(), Diagnostic> {
        let physical = self.checked_allocation_failure_jump(site, operation)?;
        self.line(&format!("spx_status = {call};"));
        self.line("if (spx_status != SPX_STATUS_SUCCESS) {");
        self.indent += 1;
        if semantic_failure {
            let semantic = self.failure_jump_at(site, StatusLane::OperationFailure)?;
            self.line("if (spx_checked_native_physical_v1(spx_ctx, spx_status)) {");
            self.indent += 1;
            for line in physical.lines() { self.line(line); }
            self.indent -= 1;
            self.line("} else {");
            self.indent += 1;
            for line in semantic.lines() { self.line(line); }
            self.indent -= 1;
            self.line("}");
        } else {
            self.line("if (!spx_checked_native_physical_v1(spx_ctx, spx_status)) spx_runtime_invariant_failure(\"total String constructor selected a semantic status\");");
            for line in physical.lines() { self.line(line); }
        }
        self.indent -= 1;
        self.line("}");
        Ok(())
    }
    pub(in crate::codegen::native_emit) fn checked_allocation_failure_jump(
        &self, site: &ExpressionId, operation: AllocationOperation,
    ) -> Result<String, Diagnostic> {
        let rows = self.function.cleanup_plan.status_sources.iter().filter(|source|
            source.id.expression == *site && source.id.lane == StatusLane::AllocationFailure);
        let mut rows = rows;
        let row = rows.next().ok_or_else(|| backend_error("checked native String has no physical source"))?;
        if rows.next().is_some() || !matches!(&row.producer,
            StatusProducer::CheckedAllocation { operation: actual, normalized_cases }
            if *actual == operation && *normalized_cases == [crate::cleanup_plan::AllocationCase::PayloadRefused,
                crate::cleanup_plan::AllocationCase::SizeOverflow]) {
            return Err(backend_error("checked native String source differs from its constructor"));
        }
        self.failure_jump_at(site, StatusLane::AllocationFailure)
    }
    pub(in crate::codegen::native_emit) fn emit_checked_string_operation(
        &mut self, op: StringOp, arguments: &[CValue], out: &str, site: &ExpressionId,
    ) -> Result<bool, Diagnostic> {
        if self.output_profile != NativeOutputProfile::CheckedAllocation { return Ok(false); }
        use AllocationOperation as A;
        let arg = |index: usize| arguments[index].code.as_str();
        let adapter = "spx_checked_native_adapter_v1(spx_ctx)";
        let (operation, call, semantic) = match op {
            StringOp::Concat => {
                let plan = self.bytes_plan.ok_or_else(|| backend_error("checked concat lacks its canonical epochs"))?;
                let mut commits = self.function.cleanup_plan.blocks.iter().flat_map(|block| block.transitions.iter()).filter_map(|transition| match transition {
                    crate::cleanup_plan::CleanupTransition::CallCommit { call, arguments } if call == site => Some(arguments),
                    _ => None,
                });
                let commit = commits.next().ok_or_else(|| backend_error("checked concat lacks CallCommit"))?;
                if commits.next().is_some() || commit.len() != 2 {
                    return Err(backend_error("checked concat group differs"));
                }
                // Authenticate both existing live epochs before retiring either.
                let (_, left, _) = plan.call_argument(site, 0)?;
                let (_, right, _) = plan.call_argument(site, 1)?;
                self.line(&format!("if (!{left} || !{right}) spx_runtime_invariant_failure(\"checked concat commit epochs\");"));
                self.line(&format!("{left} = false; {right} = false;"));
                (A::StringConcat, format!("spx_checked_native_concat_v1(spx_ctx, {}, {}, &{out})", arg(0), arg(1)), true)
            }
            StringOp::FromChar => (A::StringFromChar, format!("spx_checked_string_adapter_from_char_v1({adapter}, {}, &{out})", arg(0)), false),
            StringOp::FromI64 => (A::StringFromI64, format!("spx_checked_string_adapter_from_i64_v1({adapter}, {}, &{out})", arg(0)), false),
            StringOp::FromUsize => (A::StringFromUsize, format!("spx_checked_string_adapter_from_usize_v1({adapter}, {}, &{out})", arg(0)), false),
            StringOp::FromStr => (A::StringFromStr, format!("spx_checked_string_adapter_from_str_v1({adapter}, (const char *)({}).data, ({}).len, &{out})", arg(0), arg(0)), false),
            StringOp::FromUtf8 => (A::StringFromUtf8, format!("spx_checked_string_adapter_from_utf8_v1({adapter}, (const uint8_t *)({}).ptr, ({}).len, &{out})", arg(0), arg(0)), true),
            StringOp::Slice => (A::StringSlice, format!("spx_checked_native_slice_v1(spx_ctx, {}, {}, {}, &{out})", arg(0), arg(1), arg(2)), true),
            StringOp::Trim => (A::StringTrim, format!("spx_checked_native_trim_v1(spx_ctx, {}, &{out})", arg(0)), false),
            _ => return Ok(false),
        };
        self.checked_string_constructor(site, operation, &call, semantic)?;
        Ok(true)
    }
}
