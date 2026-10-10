//! Selected status identity validation preserves every legacy assertion while
//! charging the identity set and checking small case lists without allocation.
use super::*;
pub(super) fn validate_status_sources(
    function: &ResolvedFunction,
    expressions: &BTreeMap<ExpressionId, Option<CallFact>>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let mut ids = BTreeSet::new();
    for source in &function.cleanup_plan.status_sources {
        if !ids.contains(&source.id) {
            budget.reserve_btree_node::<StatusSourceId, ()>(function, "status identity index")?;
            budget.reserve_clone(function, &source.id, "status source identity")?;
        }
        if !ids.insert(source.id.clone()) {
            return Err(replay_error(
                function,
                format!("status source for `{}` is repeated", source.id.expression),
            ));
        }
        if !expressions.contains_key(&source.id.expression) {
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
                let Some(Some(call)) = expressions.get(&source.id.expression) else {
                    return Err(replay_error(
                        function,
                        "propagated-call status source does not name a call expression",
                    ));
                };
                if &call.callee != callee {
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
