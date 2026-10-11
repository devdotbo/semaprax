//! Primitive String descriptor access selected only by the exact slot intrinsic.
use super::super::super::{backend_error, CEmitter, COutput, CValue};
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedType};
impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn emit_string_slot(
        &mut self,
        expression: &ResolvedExpr,
    ) -> Result<CValue, Diagnostic> {
        let slot = crate::vec_ops::string_slot::inspect(expression)
            .ok_or_else(|| backend_error("invalid checked String-slot call"))?;
        let ResolvedExprKind::Place(place) = &slot.source.kind else {
            unreachable!()
        };
        let layout =
            crate::codegen::native_vec::owned_leaf::layout(self.program, &ResolvedType::String)
                .ok_or_else(|| backend_error("String-slot descriptor is unavailable"))?;
        if layout.kinds.as_slice() != [9] || layout.offsets.as_slice() != [0] {
            return Err(backend_error(
                "String-slot descriptor differs from primitive String storage",
            ));
        }
        let owner = self.emit_place(place)?;
        let index = self.emit_expr(slot.index)?;
        let plan = &self.function.cleanup_plan;
        let mut commits = plan
            .blocks
            .iter()
            .filter(|block| plan.lexical_block_in_context(self.cleanup_resume, block.id))
            .flat_map(|block| &block.transitions)
            .filter_map(|transition| match transition {
                crate::cleanup_plan::CleanupTransition::CallCommit { call, arguments }
                    if call == &expression.id =>
                {
                    Some(arguments)
                }
                _ => None,
            });
        if commits.next().is_none_or(|arguments| !arguments.is_empty()) || commits.next().is_some()
        {
            return Err(backend_error(
                "String-slot read requires one context-exact zero-owned commit",
            ));
        }
        let suffix = self.next_local;
        self.next_local += 1;
        let field = format!("spx_string_slot_{suffix}");
        self.line(&format!("const unsigned char *{field} = NULL;"));
        self.line(&format!(
            "spx_status = spx_leaf_read_field(spx_ctx, &({}), &{}, {}, UINT32_C(0), &{field});",
            owner.code, layout.symbol, index.code
        ));
        self.emit_operation_failure_check()?;
        let text = format!("spx_string_slot_text_{suffix}");
        self.line(&format!("const char *{text} = NULL;"));
        self.line(&format!("memcpy(&{text}, {field}, sizeof({text}));"));
        let output = self.temporary(&expression.ty)?;
        if slot.bytes {
            self.emit_str_as_bytes_view(
                &output,
                &CValue {
                    code: text,
                    ty: ResolvedType::String,
                },
                expression,
            )?;
        } else {
            self.line(&format!("{output} = spx_string_as_str({text});"));
        }
        let value = CValue {
            code: output,
            ty: expression.ty.clone(),
        };
        self.apply_owned_plan_at_value(&expression.id, &value)?;
        Ok(value)
    }
}
