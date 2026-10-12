//! Borrowed field uniqueness: one distinct private host import over the
//! authenticated current owner token. It never renews, consumes or replaces
//! that token, and the layout binding reuses the explicit-key metadata only.
use super::*;
pub(in crate::wasm::aggregate) const IMPORT: &str = "spx_vec_unique_by_v1";
/// Host result: 0 duplicate, 1 unique, 3 scratch refusal; anything else traps.
const SCRATCH_REFUSED: u8 = 3;
impl Emitter<'_> {
    pub(in crate::wasm::aggregate) fn emit_field_unique(
        &mut self,
        expression: &ResolvedExpr,
    ) -> Result<Value, Diagnostic> {
        let call = crate::vec_ops::field_unique::inspect(expression)
            .ok_or_else(|| error("invalid checked field uniqueness call"))?;
        let (position, _) = crate::vec_ops::field_unique::position(
            &self.program.declarations,
            call.element,
            call.identity,
        )
        .ok_or_else(|| error("field uniqueness descriptor failed replay"))?;
        let position =
            i64::try_from(position).map_err(|_| error("field uniqueness position overflow"))?;
        let (identity, shape) = super::sort_keys::metadata(self.program, call.element)?;
        let owner = self.emit_vec_borrow_place(call.source, call.element)?;
        self.apply_call_commit(&expression.id)?;
        let result = Value::Scalar {
            local: self.plan.expr_scalar(expression)?,
            ty: ResolvedType::Bool,
        };
        let local = scalar_local(&result)?;
        self.get_scalar(&owner);
        for constant in [identity, shape, position] {
            self.output.push(0x42);
            write_i64(self.output, constant);
        }
        self.output.push(0x10);
        write_u32(self.output, super::sort_keys::import_index(self.program) + 1);
        self.output.push(0x22); // local.tee result
        write_u32(self.output, local);
        self.output.extend([0x41, SCRATCH_REFUSED, 0x46]);
        self.emit_vec_failure_if(expression, STATUS_VEC_ALLOCATION_FAILURE)?;
        self.output.push(0x20);
        write_u32(self.output, local);
        self.output.extend([0x41, 0x01, 0x4b]); // any result above 1 is a broken host
        self.trap_if();
        Ok(result)
    }
}
