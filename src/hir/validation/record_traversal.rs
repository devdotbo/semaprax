//! A loop freeze must refer to an available immutable carrier in the live scope.
use super::*;
impl HirValidator<'_> {
    pub(super) fn validate_record_traversal(
        &self,
        expression: &ResolvedExpr,
        scope: &dyn ScopeRead,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutable_carrier_forgery_is_refused_by_live_hir_scope() {
        let source = r#"module app;
@id("row") record Row { @id("row.number") number:i64, }
@id("main") fn main()->i64 {
 let initial=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(initial,Row{number:7});
 for borrow row in rows { let observed=row.number; 0 }
 0
}"#;
        let program = crate::check(source, std::path::Path::new("traversal-mutable.spx"))
            .expect("original immutable source is admitted");
        let mut resolved = crate::hir::resolve(&program).unwrap();
        crate::hir::validate(&resolved).unwrap();
        let main = resolved
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "main")
            .unwrap();
        let ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
            panic!("main block");
        };
        let ResolvedStatement::Let {
            binding, mutable, ..
        } = &mut statements[1]
        else {
            panic!("carrier binding");
        };
        assert_eq!(binding.name, "rows");
        assert!(!*mutable);
        *mutable = true;
        assert_eq!(
            crate::hir::validate(&resolved).unwrap_err().code,
            "SPX-H006"
        );
    }
}
