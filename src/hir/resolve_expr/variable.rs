//! Named expression resolution, including source-only row refusal.
use super::*;
impl Resolver<'_> {
    pub(super) fn named_value(
        &self,
        function: &FunctionExecutionId,
        expr: &Expr,
        bindings: &BTreeMap<String, Binding>,
        path: &str,
        name: &str,
    ) -> Result<ResolvedExpr, Diagnostic> {
        if bindings
            .get(name)
            .is_some_and(|binding| binding.traversal_row.is_some())
        {
            return Err(self.error("SPX-H006", "record traversal row cannot escape or materialize aggregate storage; select a field", expr.span));
        }
        if let Some(reference) = self.function_reference(function, expr, bindings, path)? {
            return Ok(reference);
        }
        let binding = bindings.get(name).ok_or_else(|| {
            self.error("SPX-H002", format!("unresolved value `{name}`"), expr.span)
        })?;
        Ok(ResolvedExpr {
            id: ExpressionId::new(function, path),
            ty: binding.ty.clone(),
            ownership: binding.ownership,
            kind: ResolvedExprKind::Place(Place {
                root: binding.id.clone(),
                projections: Vec::new(),
            }),
            span: expr.span,
        })
    }
}
