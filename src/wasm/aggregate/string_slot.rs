//! Slot read uses the existing authenticated String leaf token and no copy import.
use super::*;
impl Emitter<'_> {
    pub(super) fn emit_string_slot(
        &mut self,
        expression: &ResolvedExpr,
    ) -> Result<Value, Diagnostic> {
        let slot = crate::vec_ops::string_slot::inspect(expression)
            .ok_or_else(|| error("invalid checked String-slot call"))?;
        let descriptor = vec_owned_leaf::descriptor(self.program, &ResolvedType::String)?;
        if descriptor.fields.as_slice() != [(None, ResolvedType::String)] {
            return Err(error(
                "String-slot descriptor is not the primitive String layout",
            ));
        }
        let owner = self.emit_vec_borrow_place(slot.source, &ResolvedType::String)?;
        let index = self.emit_expr(slot.index)?;
        self.require_scalar(&index, &ResolvedType::Usize, "String-slot index")?;
        self.apply_call_commit(&expression.id)?;
        let scratch = Pointer {
            local: self.plan.frame_base,
            offset: *self
                .plan
                .call_out
                .get(&expression.id)
                .ok_or_else(|| error("String-slot read scratch is absent"))?,
        };
        self.get_scalar(&owner);
        for constant in [descriptor.identity, descriptor.shape] {
            self.output.push(0x42);
            write_i64(self.output, constant);
        }
        self.get_scalar(&index);
        self.output.push(0x42);
        write_i64(self.output, 0);
        self.emit_pointer(scratch);
        self.output.push(0x10);
        write_u32(
            self.output,
            vec_owned_leaf::import_base(self.program) + vec_owned_leaf::IMPORT_COUNT,
        );
        self.output.push(0x22);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x41, 0x02, 0x46]);
        self.emit_vec_failure_if(expression, STATUS_VEC_GET_OUT_OF_BOUNDS)?;
        self.output.push(0x20);
        write_u32(self.output, self.plan.status);
        self.output.extend([0x45, 0x45]);
        self.trap_if();
        // The result is the original owned String child token. It is never a
        // raw guest address or new owner. Validate nonempty ownership before
        // the existing read-only provider authenticates generation and extent.
        self.emit_pointer(scratch);
        self.load_scalar(&ResolvedType::I64);
        self.output.extend([0x50, 0x04, 0x40, 0x05]);
        self.emit_pointer(scratch);
        self.load_scalar(&ResolvedType::I64);
        self.output.push(0x42);
        write_i64(self.output, i64::MIN);
        self.output.extend([0x83, 0x50]);
        self.trap_if();
        self.emit_pointer(scratch);
        self.load_scalar(&ResolvedType::I64);
        self.output.push(0x10);
        write_u32(self.output, BYTE_AS_SLICE_IMPORT);
        self.output.push(0x1a);
        self.output.push(0x0b);
        let result = Value::Scalar {
            local: self.plan.expr_scalar(expression)?,
            ty: expression.ty.clone(),
        };
        self.emit_pointer(scratch);
        self.load_scalar(&ResolvedType::I64);
        self.output.push(0x21);
        write_u32(self.output, scalar_local(&result)?);
        if slot.bytes {
            self.validate_byte_slice(&result);
        }
        Ok(result)
    }
}
