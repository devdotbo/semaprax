//! Actual private planner backing constructors. Legacy builds retain their
//! original unobserved allocations; caller-funded builds never create a quota.
use crate::bounded_output::cfg_reservation::CfgReservation;
use crate::diagnostic::Diagnostic;

pub(super) fn marker() -> Diagnostic {
    Diagnostic::io("SPX-H006", String::new())
}

pub(super) fn vector<T>(
    capacity: usize,
    payer: Option<&CfgReservation>,
) -> Result<Vec<T>, Diagnostic> {
    let Some(payer) = payer else { return Ok(Vec::with_capacity(capacity)); };
    payer.array::<T>(capacity).map_err(|_| marker())?;
    let mut values = Vec::new();
    if values.try_reserve_exact(capacity).is_err() {
        payer.allocation(capacity);
        return Err(marker());
    }
    if values.capacity() != capacity {
        let excess = values.capacity().saturating_sub(capacity)
            .checked_mul(std::mem::size_of::<T>()).unwrap_or(usize::MAX);
        let _ = payer.capacity(capacity, excess);
        return Err(marker());
    }
    Ok(values)
}

pub(super) fn filled<T: Clone>(
    count: usize,
    value: T,
    payer: Option<&CfgReservation>,
) -> Result<Vec<T>, Diagnostic> {
    let mut values = vector(count, payer)?;
    values.resize(count, value);
    Ok(values)
}

// This tag is selected exclusively from the actual affine owner's first
// failure, never from Diagnostic code/text (including a semantic empty H006).
#[cfg_attr(not(test), allow(dead_code, reason = "private typed outward receipt awaits complete caller activation"))]
pub(crate) enum PlannerError {
    Physical(crate::bounded_output::cfg_reservation::Refused),
    Semantic(Diagnostic),
}
pub(super) fn classify<T>(
    result: Result<T, Diagnostic>,
    payer: &CfgReservation,
) -> Result<T, PlannerError> {
    if let Some(reason) = payer.first() { return Err(PlannerError::Physical(reason)); }
    result.map_err(PlannerError::Semantic)
}
