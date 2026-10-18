//! Exact selected structural twin; complete original remains in the parent owner.
use super::*;
use crate::cleanup_plan::replay::semantic_refusal::Error as _;
pub(in crate::cleanup_plan::replay) fn validate_core(
    function: &ResolvedFunction,
    expressions: &dyn expression_facts::Lookup,
    budget: &mut ReplayBudget,
) -> Result<(), Error> {
    // Actual formal-argument/local slots; existing prepaid backing/cursor
    // headers remain independently charged at their original formation sites.
    type Receiving<'a> = (
        &'a ResolvedFunction,
        &'a dyn expression_facts::Lookup,
        &'a mut ReplayBudget,
        BTreeSet<StatusSourceId>,
        &'a StatusSource,
        bool,
        &'a CallFact<'a>,
        bool,
        bool,
        usize,
        &'a StatusCase,
        std::slice::Iter<'a, StatusSource>,
    );
    budget.reserve_array::<Receiving<'_>>(function, 1, "selected validate_core local receiving")?;
    budget.charge(function, 12, "selected validate_core local receiving")?;

    let permit = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
        budget,
        function,
        crate::cleanup_plan::replay::selected_context::child::FamilyCallReceiving<'_>,
        3,
        "selected control permit call receiving",
        Permit::control_receiver::<()>(function, budget)
    )?;
    let mut ids = BTreeSet::new();
    #[cfg(test)]
    let mut borrowed_ids = if expressions.is_metered() {
        Some(structural_control::borrowed_ids::Ids::new(
            function, budget,
        )?)
    } else {
        None
    };
    for source in &function.cleanup_plan.status_sources {
        #[cfg(test)]
        let inserted = match &mut borrowed_ids {
            Some(ids) => ids.insert(
                function,
                structural_control::borrowed_ids::Key::Status(&source.id),
                budget,
            )?,
            None => {
                if !ids.contains(&source.id) {
                    budget.reserve_btree_node::<StatusSourceId, ()>(
                        function,
                        "status identity index",
                    )?;
                    budget.reserve_clone(function, &source.id, "status source identity")?;
                }
                ids.insert(source.id.clone())
            }
        };
        #[cfg(not(test))]
        let inserted = {
            if !ids.contains(&source.id) {
                budget
                    .reserve_btree_node::<StatusSourceId, ()>(function, "status identity index")?;
                budget.reserve_clone(function, &source.id, "status source identity")?;
            }
            ids.insert(source.id.clone())
        };
        if !inserted {
            return Err(Error::structural(
                function,
                semantic_refusal::Atom::RepeatedStatus(source.id.expression.clone()),
            ));
        }
        if expressions
            .fact(function, &source.id.expression, budget)?
            .is_none()
        {
            return Err(Error::structural(
                function,
                semantic_refusal::Atom::UnknownStatus(source.id.expression.clone()),
            ));
        }
        match (&source.id.lane, &source.producer) {
            (
                StatusLane::OperationFailure | StatusLane::OwnerAdmission,
                StatusProducer::PropagatedCall { callee },
            ) => {
                let Some(Some(call)) = crate::cleanup_plan::replay::inventory_coverage::paid::receive!(
                    budget,
                    function,
                    (
                        &dyn expression_facts::Lookup,
                        &ResolvedFunction,
                        &ExpressionId,
                        &mut ReplayBudget,
                        Result<Option<&Option<CallFact<'_>>>, Diagnostic>
                    ),
                    5,
                    "selected control expressions fact call receiving",
                    expressions.fact(function, &source.id.expression, budget)
                )?
                else {
                    return Err(permit.control(Reason::Finite(
                        Kind::PropagatedCallStatusSourceDoesNotNameACallExpression,
                    )));
                };
                #[cfg(test)]
                let agrees = if expressions.is_metered() {
                    schema::borrowed::same_schema(
                        function,
                        call.callee.as_str(),
                        callee.as_str(),
                        budget,
                    )?
                } else {
                    call.callee.as_ref() == callee
                };
                #[cfg(not(test))]
                let agrees = call.callee.as_ref() == callee;
                if !agrees {
                    return Err(permit.control(Reason::Finite(
                        Kind::PropagatedCallStatusSourceNamesTheWrongCallee,
                    )));
                }
            }
            (
                StatusLane::OperationFailure,
                StatusProducer::CheckedArithmetic {
                    normalized_cases, ..
                },
            ) => {
                let mut repeated = false;
                for (index, case) in normalized_cases.iter().enumerate() {
                    budget.charge(function, index, "checked arithmetic case uniqueness")?;
                    repeated |= normalized_cases[..index].contains(case);
                }
                if normalized_cases.is_empty() || repeated {
                    return Err(permit.control(Reason::Finite(
                        Kind::CheckedArithmeticStatusCasesAreEmptyOrRepeated,
                    )));
                }
            }
            (StatusLane::ContractFalse, StatusProducer::ContractFalse { .. }) => {}
            _ => {
                return Err(permit.control(Reason::Finite(
                    Kind::StatusSourceLaneDoesNotMatchItsProducer,
                )));
            }
        }
    }
    Ok(())
}
