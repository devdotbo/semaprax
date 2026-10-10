//! A private primitive selected entry owns one cumulative reservation.
//! Independent replay reconstructs every fact from the immutable context; no
//! attached schema or certificate is an admission shortcut.
use super::*;

#[cfg(test)]
pub(crate) fn validate_retained_lexical_with_limit(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    bytes: usize,
) -> Result<(), Diagnostic> {
    validate_retained_lexical_with_reservation(program, function, PhysicalReservation::new(bytes))
        .map(|_| ())
}

pub(crate) fn validate_retained_lexical_with_reservation(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    physical: PhysicalReservation,
) -> Result<PhysicalReservation, Diagnostic> {
    if !crate::hir::lexical_exit::requires(function)?
        || !matches!(
            function.cleanup_plan.schema,
            super::super::CLEANUP_PLAN_SCHEMA_V18 | super::super::CLEANUP_PLAN_SCHEMA_V19
        )
    {
        return Err(replay_error(
            function,
            "selected retained lexical replay requires an actual lexical function",
        ));
    }
    let mut budget = ReplayBudget::from_reservation(function, physical)?;
    {
        let indexed = selected_callees::Context::new(program, function, &mut budget)?;
        let context = selected_context::Context::new(&indexed, function, &mut budget)?;
        lexical_replay::validate_with_index(&context, function, &mut budget, None)?;
    }
    // All per-function constructors have dropped before the ledger is returned.
    // The debit is cumulative: this is no refund or phase reset.
    let physical = budget
        .physical
        .take()
        .ok_or_else(|| replay_error(function, "selected replay lost its physical reservation"))?;
    Ok(*physical)
}
