//! Complete independently executable original current builder receiver.
use super::*;

pub(super) fn with_budget<'hir>(
    function: &'hir ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<BTreeMap<ExpressionId, Option<CallFact<'hir>>>, Diagnostic> {
    let builder_payer = match budget.payer {
        AllocationPayer::ActiveBuilder => Some(capacity::Payer::ActiveBuilder),
        #[cfg(test)]
        AllocationPayer::Oracle(lease) => Some(capacity::Payer::Oracle(lease)),
        _ => None,
    };
    if let Some(builder_payer) = builder_payer {
        budget.prepare_builder(function)?;
        let result = with_capacity::<errors::Selected>(function, builder_payer);
        return match result {
            Err(errors::EngineError::Fact(receipt)) if receipt.physical() => {
                Err(budget.budget_refusal(function, receipt.detail(), ""))
            }
            Err(errors::EngineError::Fact(receipt)) => Err(receipt.legacy(function)),
            Err(errors::EngineError::Owned(error)) => Err(error),
            Err(errors::EngineError::Structural(receipt)) => Err(receipt.legacy(function)),
            Err(errors::EngineError::Failed) => {
                Err(budget.budget_refusal(function, "expression facts already failed", ""))
            }
            Ok(facts) => Ok(facts),
        };
    }
    match budget.physical.as_deref_mut() {
        Some(physical) => expression_facts_with_reservation(function, physical),
        None => expression_facts(function),
    }
}
