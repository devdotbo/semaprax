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
    require_entry(function)?;
    let mut budget = ReplayBudget::from_reservation(function, physical)?;
    execute(program, function, &mut budget)?;
    // All per-function constructors have dropped before the ledger is returned.
    // The debit is cumulative: this is no refund or phase reset.
    let physical = budget
        .physical
        .take()
        .ok_or_else(|| replay_error(function, "selected replay lost its physical reservation"))?;
    Ok(*physical)
}

fn require_entry(function: &ResolvedFunction) -> Result<(), Diagnostic> {
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
    Ok(())
}
fn execute(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let indexed = selected_callees::Context::new(program, function, budget)?;
    let context = selected_context::Context::new(&indexed, function, budget)?;
    lexical_replay::validate_with_index(&context, function, budget, None)
}

/// Whole-entry unit oracle uses the same constructors and proof owner as the
/// selected production entry, with independently supplied exact work/bytes.
#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn validate_with_limits(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    work: usize,
    bytes: usize,
) -> Result<(usize, usize), Diagnostic> {
    require_entry(function)?;
    let mut budget = ReplayBudget::with_physical(function, bytes)?;
    budget.remaining = work;
    execute(program, function, &mut budget)?;
    let spent = work
        .checked_sub(budget.remaining)
        .expect("work never increases");
    let charged = budget
        .physical
        .as_ref()
        .expect("selected reservation retained")
        .charged();
    // Drop the whole budget and all constructors inside the caller's observer.
    drop(budget);
    Ok((spent, charged))
}
