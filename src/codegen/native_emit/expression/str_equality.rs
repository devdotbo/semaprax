//! Exact length-delimited equality without creating or transferring an owner.
use super::{BinaryOp, CEmitter, COutput, CValue, Diagnostic, ResolvedExpr, ResolvedType};

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(super) fn emit_str_equality(
        &mut self,
        op: BinaryOp,
        left: CValue,
        right: &ResolvedExpr,
        result_type: &ResolvedType,
    ) -> Result<CValue, Diagnostic> {
        let right = self.emit_expr(right)?;
        self.require_type(&left.ty, &ResolvedType::Str, "Str equality left")?;
        self.require_type(&right.ty, &ResolvedType::Str, "Str equality right")?;
        self.require_type(result_type, &ResolvedType::Bool, "Str equality result")?;
        let result = self.temporary(&ResolvedType::Bool)?;
        self.line(&format!("spx_str_require_valid({});", left.code));
        self.line(&format!("spx_str_require_valid({});", right.code));
        self.line(&format!(
            "{result} = {}({}.len == {}.len && ({}.len == 0 || memcmp({}.data, {}.data, (size_t){}.len) == 0));",
            if op == BinaryOp::Ne { "!" } else { "" },
            left.code, right.code, left.code, left.code, right.code, left.code,
        ));
        Ok(CValue {
            code: result,
            ty: ResolvedType::Bool,
        })
    }
}
