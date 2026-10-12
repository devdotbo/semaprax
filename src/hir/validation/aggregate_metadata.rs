//! Borrow current indexed declaration metadata through the complete frame lifetime.
//! No resolved type substitution output or mutable scope state is borrowed here.
use super::*;
#[cfg(test)]
mod tests;

pub(super) fn record_fields<'a>(
    program: &'a ResolvedProgram,
    record: &DeclarationId,
) -> Option<&'a [ResolvedFieldDeclaration]> {
    program.declarations.record_fields(record)
}
pub(super) fn variant_cases<'a>(
    program: &'a ResolvedProgram,
    variant: &DeclarationId,
) -> Option<&'a [ResolvedVariantCaseDeclaration]> {
    program.declarations.variant_cases(variant)
}
