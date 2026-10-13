//! Original logical sink policy with actual work on the held replay payer.
use super::*;
use crate::bounded_output::classifier_string::{ClassifierString, GrowthFailure};
use std::fmt;

fn work(units: usize, payer: &mut Payer<'_>) -> Result<(), Diagnostic> {
    inventory_text::current(payer)?;
    payer
        .budget
        .charge(payer.function, units, "inventory classifier text work")
}
fn pay(bytes: usize, payer: &mut Payer<'_>) -> Result<(), Diagnostic> {
    inventory_text::current(payer)?;
    payer.bytes(bytes)
}
pub(super) fn acquire(
    payer: &mut Payer<'_>,
) -> Result<ClassifierString, GrowthFailure<Diagnostic>> {
    ClassifierString::acquire_with_work(payer, &mut pay, &mut work)
}
pub(super) fn capped(
    output: &mut ClassifierString,
    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    output.capped_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn strict(
    output: &mut ClassifierString,
    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    output.strict_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn raw(
    output: &mut ClassifierString,
    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    output.raw_with_work(arguments, payer, &mut pay, &mut work)
}

#[cfg(test)]
mod tests;
