//! Borrowed record-field uniqueness: replayed static field metadata, one
//! borrowed named owner, and runtime ordinal scratch only.
use super::super::super::{backend_error, CEmitter, COutput, CValue};
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedExprKind, ResolvedType};
impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn emit_field_unique(
        &mut self,
        expression: &ResolvedExpr,
    ) -> Result<CValue, Diagnostic> {
        let call = crate::vec_ops::field_unique::inspect(expression)
            .ok_or_else(|| backend_error("invalid checked field uniqueness call"))?;
        let (position, key) = crate::vec_ops::field_unique::position(
            &self.program.declarations,
            call.element,
            call.identity,
        )
        .ok_or_else(|| backend_error("field uniqueness descriptor failed independent replay"))?;
        let ResolvedExprKind::Place(place) = &call.source.kind else {
            unreachable!()
        };
        self.require_type(&expression.ty, &ResolvedType::Bool, "field uniqueness result")?;
        // Borrow the named owner directly; generic owning lowering would stage
        // a transfer that this read-only operation must never perform.
        let owner = self.emit_place(place)?;
        self.require_type(&owner.ty, &call.source.ty, "field uniqueness source place")?;
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
                "field uniqueness requires one context-exact zero-owned commit",
            ));
        }
        let output = self.temporary(&ResolvedType::Bool)?;
        if let Some(fields) =
            crate::hir::copy_record_collection::fields(&self.program.declarations, call.element)
        {
            if fields.get(position).is_none_or(|field| field.ty != key) {
                return Err(backend_error("field uniqueness Copy layout disagrees"));
            }
            let width = fields.len();
            let tag = super::vec_element_tag(&key)?;
            self.line(&format!(
                "spx_status = spx_copy_vec_unique_by(spx_ctx, &({}), UINT64_C({width}), UINT32_C({position}), UINT32_C({tag}), &{output});",
                owner.code
            ));
        } else {
            let layout = crate::codegen::native_vec::owned_leaf::layout(self.program, call.element)
                .ok_or_else(|| backend_error("field uniqueness has no native row descriptor"))?;
            let expected = crate::hir::owned_leaf_collection::layout(
                &self.program.declarations,
                call.element,
            )
            .and_then(|layout| layout.fields)
            .map_or(0, <[_]>::len);
            if layout.kinds.len() != expected || position >= layout.kinds.len() {
                return Err(backend_error(
                    "field uniqueness declaration order disagrees with native layout",
                ));
            }
            self.line(&format!(
                "spx_status = spx_leaf_unique_by(spx_ctx, &({}), &{}, UINT32_C({position}), &{output});",
                owner.code, layout.symbol
            ));
        }
        self.emit_operation_failure_check()?;
        let value = CValue {
            code: output,
            ty: ResolvedType::Bool,
        };
        self.apply_owned_plan_at_value(&expression.id, &value)?;
        Ok(value)
    }
}
