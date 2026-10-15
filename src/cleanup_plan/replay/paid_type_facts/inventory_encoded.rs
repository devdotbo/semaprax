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
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return ClassifierString::acquire_oracle_work(payer, &mut pay, &mut work);
    }
    ClassifierString::acquire_with_work(payer, &mut pay, &mut work)
}
pub(super) fn capped(
    output: &mut ClassifierString,
    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return output.write_oracle_work(
            arguments,
            payer,
            &mut pay,
            &mut |bytes, payer: &mut Payer<'_>| payer.classifier_reserve_sink(bytes),
            &mut work,
            false,
        );
    }
    output.capped_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn strict(
    output: &mut ClassifierString,
    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return output.write_oracle_work(
            arguments,
            payer,
            &mut pay,
            &mut |bytes, payer: &mut Payer<'_>| payer.classifier_reserve_sink(bytes),
            &mut work,
            true,
        );
    }
    output.strict_with_work(arguments, payer, &mut pay, &mut work)
}
pub(super) fn raw(
    output: &mut ClassifierString,
    arguments: fmt::Arguments<'_>,
    payer: &mut Payer<'_>,
) -> Result<(), GrowthFailure<Diagnostic>> {
    #[cfg(test)]
    if payer.budget.oracle_lease().is_some() {
        return output.write_oracle_work(
            arguments,
            payer,
            &mut pay,
            &mut |_, payer: &mut Payer<'_>| {
                inventory_text::current(payer)?;
                Ok(true)
            },
            &mut work,
            false,
        );
    }
    output.raw_with_work(arguments, payer, &mut pay, &mut work)
}

#[cfg(test)]
mod tests;

impl crate::hir::InventoryPayer for Payer<'_> {
    fn inventory_work(&mut self, units: usize, phase: &'static str) -> Result<(), Diagnostic> {
        inventory_text::current(self)?;
        self.budget.charge(self.function, units, phase)
    }
    fn inventory_clone(&mut self, ty: &ResolvedType) -> Result<ResolvedType, Diagnostic> {
        inventory_clone::clone_type(ty, self)
    }
}
