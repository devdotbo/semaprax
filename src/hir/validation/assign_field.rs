//! Exact direct Copy-field mutation validation, relocated without policy changes.
use super::{
    hir_diagnostic_at_span, hir_error_at_span, DeclarationId, DeclarationKind, Diagnostic,
    HirValidator, ResolvedExpr, ResolvedType,
};
impl HirValidator<'_> {
    /// Field Mutation v1 oracle check: the targeted field must exist on a
    /// record/class base, stay a direct scalar Copy field, and match the
    /// assigned value's type exactly.
    pub(super) fn validate_assign_field(
        &self,
        target_ty: &ResolvedType,
        field: &DeclarationId,
        assigned: &ResolvedExpr,
        span: crate::ast::Span,
    ) -> Result<(), Diagnostic> {
        let ResolvedType::Nominal {
            declaration: owner,
            arguments,
        } = target_ty
        else {
            return Err(hir_error_at_span(
                span,
                "field assignment base is not a record",
            ));
        };
        if self
            .program
            .declarations
            .declaration(owner)
            .is_none_or(|item| {
                !matches!(item.kind, DeclarationKind::Record | DeclarationKind::Class)
            })
        {
            return Err(hir_error_at_span(
                span,
                "field assignment base is not a record",
            ));
        }
        let declared = self
            .program
            .declarations
            .record_fields(owner)
            .and_then(|fields| fields.iter().find(|item| &item.id == field))
            .map(|item| item.ty.clone())
            .ok_or_else(|| {
                hir_error_at_span(
                    span,
                    format!("record `{owner}` has no assignment field `{field}`"),
                )
            })?;
        let field_ty =
            crate::hir::substitute_type(&declared, owner, arguments).map_err(|diagnostic| {
                hir_error_at_span(
                    span,
                    format!("assignment field type substitution failed: {diagnostic}"),
                )
            })?;
        self.require_type(&field_ty, &assigned.ty, "field assignment")
            .map_err(|diagnostic| hir_diagnostic_at_span(diagnostic, span))?;
        if !crate::hir::is_scalar_resolved_type(&field_ty) {
            return Err(hir_error_at_span(
                span,
                "field mutation v1 supports only direct scalar Copy record fields",
            ));
        }
        Ok(())
    }
}
