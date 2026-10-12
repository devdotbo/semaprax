//! Selected status identity validation preserves every legacy assertion while
//! charging the identity set and checking small case lists without allocation.
use super::*;
#[cfg(test)]
pub(super) fn validate_status_sources(
    function: &ResolvedFunction,
    expressions: &dyn expression_facts::Lookup,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    validate_core::<Diagnostic>(function, expressions, budget)
}

pub(super) fn validate_core<E: semantic_refusal::Error>(
    function: &ResolvedFunction,
    expressions: &dyn expression_facts::Lookup,
    budget: &mut ReplayBudget,
) -> Result<(), E> {
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
            return Err(E::structural(
                function,
                semantic_refusal::Atom::RepeatedStatus(source.id.expression.clone()),
            ));
        }
        if expressions
            .fact(function, &source.id.expression, budget)?
            .is_none()
        {
            return Err(E::structural(
                function,
                semantic_refusal::Atom::UnknownStatus(source.id.expression.clone()),
            ));
        }
        match (&source.id.lane, &source.producer) {
            (
                StatusLane::OperationFailure | StatusLane::OwnerAdmission,
                StatusProducer::PropagatedCall { callee },
            ) => {
                let Some(Some(call)) = expressions.fact(function, &source.id.expression, budget)?
                else {
                    return Err(replay_error(
                        function,
                        "propagated-call status source does not name a call expression",
                    )
                    .into());
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
                    return Err(replay_error(
                        function,
                        "propagated-call status source names the wrong callee",
                    )
                    .into());
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
                    return Err(replay_error(
                        function,
                        "checked-arithmetic status cases are empty or repeated",
                    )
                    .into());
                }
            }
            (StatusLane::ContractFalse, StatusProducer::ContractFalse { .. }) => {}
            _ => {
                return Err(replay_error(
                    function,
                    "status source lane does not match its producer",
                )
                .into());
            }
        }
    }
    Ok(())
}
