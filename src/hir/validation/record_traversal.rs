//! A loop freeze must refer to an available immutable carrier in the live scope.
use super::*;
impl HirValidator<'_> {
    pub(super) fn validate_record_traversal(
        &self,
        expression: &ResolvedExpr,
        scope: &BTreeMap<ValueId, ValidationBinding>,
    ) -> Result<(), Diagnostic> {
        let Some(proof) = crate::loan_plan::traversal::inspect(self.program, expression)? else {
            return Ok(());
        };
        let ResolvedExprKind::Place(place) = &proof.source.kind else {
            return Err(hir_error("record traversal source is not an exact place"));
        };
        let binding = scope
            .get(&place.root)
            .ok_or_else(|| hir_error("record traversal source is absent from its live scope"))?;
        let (ty, ownership) = self.resolve_place(place, binding)?;
        if binding.mutable
            || !place.projections.is_empty()
            || ty != proof.source.ty
            || ownership != proof.source.ownership
            || Self::place_availability(binding, &[]) != Availability::Available
        {
            return Err(hir_error(
                "record traversal source must be an available immutable whole carrier",
            ));
        }
        Ok(())
    }
}
