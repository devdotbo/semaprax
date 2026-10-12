//! Private funding route pending actual canonical caller handoff. Legacy
//! semantic diagnostics and upstream compiler preflight remain outside this
//! storage protocol; canonical callers must not activate it until reviewed.
use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;
use super::storage::{classify, PlannerError};

#[cfg_attr(not(test), allow(dead_code, reason = "private partial flat-storage receiver; full caller inventory payment is not yet activated"))]
pub(super) fn build(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    payer: &CfgReservation,
) -> Result<LoanPlan, PlannerError> {
    // Same immutable current subject, with no new source/profile authority.
    if !program.functions.iter().chain(program.function_instances.iter().map(|i| &i.function))
        .any(|current| std::ptr::eq(current, function))
    {
        return Err(PlannerError::Semantic(error("funded loan planner requires the exact current function")));
    }
    let result = (|| {
        payer.reserve(0).map_err(|_| storage::marker())?;
        let lexical = crate::hir::lexical_exit::requires(function)?;
        if lexical {
            let owner = crate::hir::lexical_exit::owner(program, function)?;
            crate::hir::lexical_exit::validate_function(program, function, &owner)?;
        }
        if paid_candidate::inspect(program, function, payer)? {
            let mut work = WorkCounter::new(MAX_LOAN_PLAN_WORK_V1);
            work.allocation = Some(payer);
            paid_plan::build(program, function, &mut work)
        } else {
            let mut plan = LoanPlan::empty_v1();
            if lexical { plan.schema = LOAN_PLAN_SCHEMA_V3; }
            Ok(plan)
        }
    })();
    // Guidance is semantic rendering and may allocate. Never run it on a
    // physical marker or use its text to decide which receipt to publish.
    match classify(result, payer) {
        Err(PlannerError::Semantic(error)) =>
            Err(PlannerError::Semantic(guidance::work_limit(error, function))),
        result => result,
    }
}

#[cfg(test)]
mod tests;
