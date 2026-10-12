//! Constructor-local paid identity directory insertion.
use super::*;
#[cfg(test)]
pub(super) mod coarse;
pub(super) fn expression(
    values: &mut identity_directory::Set<ExpressionId>,
    id: &ExpressionId,
) -> Result<bool, Diagnostic> {
    values.insert_paid(id)
}
pub(super) fn value(
    values: &mut identity_directory::Set<ValueId>,
    id: &ValueId,
) -> Result<bool, Diagnostic> {
    values.insert_paid(id)
}
#[cfg(test)]
mod tests;
