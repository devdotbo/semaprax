//! Private source-checked partition lowering. Ordinary source admission stays closed.
use super::*;

pub(super) fn uses(program: &ResolvedProgram) -> bool {
    program.functions.iter().any(|function| function.requires.iter()
        .chain(std::iter::once(&function.body)).chain(&function.ensures).any(|root| {
        let mut cursors = [None; 514];
        cursors[0] = Some(crate::hir::lexical_exit::ChildCursor::new(root));
        let mut depth = 1;
        let mut current = Some(root);
        while depth > 0 {
            if current.take().is_some_and(|expr| matches!(&expr.kind,
                ResolvedExprKind::Call { callee, .. } if callee.as_str() == crate::construction_fusion::partition::ID)) {
                return true;
            }
            if let Some(child) = cursors[depth - 1].as_mut().unwrap().next() {
                // Complete compiler validation has already bounded this depth.
                if depth == cursors.len() { return true; }
                cursors[depth] = Some(crate::hir::lexical_exit::ChildCursor::new(child));
                depth += 1;
                current = Some(child);
            } else { depth -= 1; }
        }
        false
    }))
}

impl Emitter<'_> {
    pub(super) fn emit_byte_partition(
        &mut self, expr: &ResolvedExpr, values: &[Value],
    ) -> Result<Value, Diagnostic> {
        let [owner, count, width] = values else { return Err(error("partition operand census")); };
        let index = *self.function_indexes.get(&FunctionExecutionId::Monomorphic(
            DeclarationId::new(crate::construction_fusion::partition::ID)))
            .ok_or_else(|| error("partition producer is not current-module indexed"))?;
        let result = self.plan.expr_scalar(expr)?;
        self.get_scalar(owner);
        self.get_scalar(count);
        self.get_scalar(width);
        self.output.push(0x10); write_u32(self.output, index);
        // Exact import results are (Vec handle, status). Failure leaves owner live.
        self.output.push(0x21); write_u32(self.output, self.plan.status);
        self.output.push(0x21); write_u32(self.output, result);
        self.output.push(0x20); write_u32(self.output, self.plan.status);
        self.output.extend([0x04, 0x40]);
        // The private status namespace preserves the actual operation code,
        // and cannot collide with any ordinary arithmetic/Vec status.
        self.output.push(0x20); write_u32(self.output, self.plan.status);
        self.output.push(0x41); write_i64(self.output, 0x7000);
        self.output.push(0x6a);
        self.output.push(0x21); write_u32(self.output, self.plan.status);
        self.emit_failure_cleanup(&expr.id, StatusLane::OperationFailure)?;
        self.output.push(0x0c);
        write_u32(self.output, self.control_depth + self.status_exit_extra_depth);
        self.output.push(0x0b);
        // Only the successful host transfer consumes the staged argument.
        self.apply_call_commit(&expr.id)?;
        self.clear_scalar(owner)?;
        Ok(Value::Scalar { local: result, ty: expr.ty.clone() })
    }
}
