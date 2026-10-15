//! Row Loan v1 in the reference interpreter: the helper's borrowed parameter
//! aliases the exact frozen Vec row. No record, leaf or Vec is cloned, moved
//! or materialized; the Arc alias ends when the call frame returns.
use super::*;

impl Evaluator<'_> {
    pub(super) fn evaluate_row_loan(
        &mut self,
        parameter: &hir::ResolvedParam,
        loan: &hir::RowLoan,
        args: &[ResolvedExpr],
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        self.charge()?;
        if loan.version != hir::RowLoanVersion::V1
            || parameter.id != loan.parameter
            || parameter.ownership != hir::OwnershipMode::Borrow
            || parameter.ty != loan.element
        {
            return Err(Flow::Guard(
                "row loan disagrees with its exact borrowed parameter",
            ));
        }
        let [source, index] = args else {
            return Err(Flow::Guard("invalid row loan arity"));
        };
        let ResolvedExprKind::Place(place) = &source.kind else {
            return Err(Flow::Guard("row loan has no named Vec carrier"));
        };
        if !place.projections.is_empty() {
            return Err(Flow::Guard("row loan carrier is projected"));
        }
        // The whole Vec is inspected before the index, left to right. The
        // carrier reference retains no semantic owner and renews nothing.
        let Some(Value::Vec(vector)) = environment.get(&place.root) else {
            return Err(Flow::Guard("row loan Vec carrier is not live"));
        };
        if vector.element != loan.element {
            return Err(Flow::Guard("row loan element identity mismatch"));
        }
        let vector = Arc::clone(vector);
        let generation = vector.generation;
        let Value::Usize(index) = self.evaluate(index, environment, depth)? else {
            return Err(Flow::Guard("row loan index is not usize"));
        };
        // The loop freeze keeps this exact generation for the complete call.
        let Some(Value::Vec(current)) = environment.get(&place.root) else {
            return Err(Flow::Guard("row loan Vec carrier disappeared"));
        };
        if !Arc::ptr_eq(current, &vector) || current.generation != generation {
            return Err(Flow::Guard("row loan Vec generation is stale"));
        }
        let row = usize::try_from(index)
            .ok()
            .and_then(|index| vector.values.get(index))
            .ok_or(Flow::Guard(
                "row loan index exceeds its frozen traversal bound",
            ))?;
        if !owned_vec::element_value_matches_type(self.declarations, row, &loan.element) {
            return Err(Flow::Guard("row loan row is unauthenticated"));
        }
        let Value::Record(record) = row else {
            return Err(Flow::Guard("row loan row is not a record"));
        };
        Ok(Value::Record(Arc::clone(record)))
    }
}
