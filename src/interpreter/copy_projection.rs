//! Scalar projections of independently admitted flat Copy record carriers.
use super::*;

pub(super) fn admitted(program: &ResolvedProgram, expression: &ResolvedExpr) -> bool {
    let ResolvedExprKind::Project { base, field } = &expression.kind else {
        return false;
    };
    base.ownership == OwnershipMode::Value
        && expression.ownership == OwnershipMode::Value
        && hir::copy_record_collection::fields(&program.declarations, &base.ty).is_some_and(
            |fields| {
                fields
                    .iter()
                    .any(|item| item.id == *field && item.ty == expression.ty)
            },
        )
}

impl Evaluator<'_> {
    pub(super) fn evaluate_copy_projection(
        &mut self,
        expression: &ResolvedExpr,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        let ResolvedExprKind::Project { base, field } = &expression.kind else {
            return Err(Flow::Guard("Copy projection shape"));
        };
        let Value::Record(record) = self.evaluate(base, environment, depth)? else {
            return Err(Flow::Guard("Copy projection carrier"));
        };
        let value = record
            .fields
            .iter()
            .find(|(id, _)| id == field)
            .map(|(_, value)| value)
            .ok_or(Flow::Guard("Copy projection field"))?;
        self.clone_value(value)
    }
}
