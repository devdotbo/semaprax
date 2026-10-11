//! Selected status identity validation preserves every legacy assertion while
//! charging the identity set and checking small case lists without allocation.
use super::*;
pub(super) fn validate_status_sources(
    function: &ResolvedFunction,
    expressions: &dyn expression_facts::Lookup,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
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
            return Err(replay_error(
                function,
                format!("status source for `{}` is repeated", source.id.expression),
            ));
        }
        if expressions
            .fact(function, &source.id.expression, budget)?
            .is_none()
        {
            return Err(replay_error(
                function,
                format!(
                    "status source references unknown expression `{}`",
                    source.id.expression
                ),
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
                    ));
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
                    &call.callee == callee
                };
                #[cfg(not(test))]
                let agrees = &call.callee == callee;
                if !agrees {
                    return Err(replay_error(
                        function,
                        "propagated-call status source names the wrong callee",
                    ));
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
                    ));
                }
            }
            (StatusLane::ContractFalse, StatusProducer::ContractFalse { .. }) => {}
            _ => {
                return Err(replay_error(
                    function,
                    "status source lane does not match its producer",
                ));
            }
        }
    }
    Ok(())
}
