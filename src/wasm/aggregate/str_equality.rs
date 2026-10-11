//! Compare actual authenticated borrowed text, never packed pointer identity.
use super::{
    error, require_type, value_type, write_u32, BinaryOp, DeclarationId, Diagnostic, Emitter,
    FunctionExecutionId, ResolvedType, Value,
};

impl Emitter<'_> {
    pub(super) fn emit_borrowed_str_equality(
        &mut self,
        op: BinaryOp,
        left: &Value,
        right: &Value,
        destination: u32,
    ) -> Result<(), Diagnostic> {
        require_type(value_type(left), &ResolvedType::Str, "Str equality left")?;
        require_type(value_type(right), &ResolvedType::Str, "Str equality right")?;
        // The same predicate receiver as str_starts_with authenticates both
        // carriers even when their lengths differ. Tagged owners must never
        // be rotated into a guest-memory pointer.
        let helper = if let Some(index) =
            self.owned_str_predicate_pair(crate::str_ops::StrOp::StartsWith, left, right)
        {
            index
        } else if let Some(index) = self.environment_utf8_index {
            self.emit_text_helper_view(left);
            self.emit_text_helper_view(right);
            index + 1
        } else {
            self.emit_text_helper_view(left);
            self.emit_text_helper_view(right);
            self.function_indexes
                .get(&FunctionExecutionId::Monomorphic(DeclarationId::new(
                    crate::str_ops::STARTS_WITH_ID,
                )))
                .copied()
                .ok_or_else(|| error("Str equality predicate is not indexed"))?
        };
        self.output.push(0x10);
        write_u32(self.output, helper);
        self.get_scalar(left);
        self.output.push(0xa7);
        self.get_scalar(right);
        self.output.extend([0xa7, 0x46, 0x71]); // equal byte lengths AND prefix
        if op == BinaryOp::Ne {
            self.output.push(0x45);
        }
        self.output.push(0x21);
        write_u32(self.output, destination);
        Ok(())
    }
}
