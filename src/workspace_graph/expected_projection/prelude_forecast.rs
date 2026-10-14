//! Base prelude HIR input missing from authored AST forecasting. This uses
//! borrowed producer metadata and existing structural/string/identity factors.
//! It bounds the unconditional prefix only; extended selected declarations
//! and independent constructor/replay terms retain their existing accounting.
use super::{
    active_builder_limit, checked_builder_sum, limit_error, StructuralCost,
    HIR_IDENTITY_COPY_FACTOR, HIR_STRING_COPY_FACTOR, HIR_STRUCTURE_EXPANSION_FACTOR,
};
use crate::ast::{
    FieldDeclaration, Type, TypeDeclaration, TypeParameterDeclaration, VariantCaseDeclaration,
};
use crate::diagnostic::Diagnostic;
use crate::prelude::base_schema::Variant;

fn input_cost(selected: &[&Variant]) -> Result<(StructuralCost, usize), Vec<Diagnostic>> {
    let mut cost = StructuralCost::new();
    let mut slots = 0;
    for declaration in selected {
        cost.add(std::mem::size_of::<TypeDeclaration>())?;
        cost.string(declaration.id)?;
        cost.string(declaration.name)?;
        for parameter in declaration.parameters {
            cost.add(std::mem::size_of::<TypeParameterDeclaration>())?;
            cost.string(parameter)?;
        }
        for case in declaration.cases {
            cost.add(std::mem::size_of::<VariantCaseDeclaration>())?;
            cost.string(case.id)?;
            cost.string(case.name)?;
            for field in case.fields {
                cost.add(std::mem::size_of::<FieldDeclaration>())?;
                cost.string(field.id)?;
                cost.string(field.name)?;
                cost.add(std::mem::size_of::<Type>())?;
                cost.string(field.parameter)?;
                slots = checked_builder_sum(slots, 1)?;
            }
        }
    }
    Ok((cost, slots))
}

pub(super) fn retained_upper(
    selected: &[&Variant],
    maximum_identity: usize,
) -> Result<usize, Vec<Diagnostic>> {
    let (cost, slots) = input_cost(selected)?;
    let maximum_identity = selected
        .iter()
        .flat_map(|schema| {
            std::iter::once(schema.id).chain(schema.cases.iter().flat_map(|case| {
                std::iter::once(case.id).chain(case.fields.iter().map(|field| field.id))
            }))
        })
        .map(str::len)
        .fold(maximum_identity, usize::max);
    let bytes = cost
        .total
        .checked_sub(cost.string_bytes)
        .and_then(|n| n.checked_mul(HIR_STRUCTURE_EXPANSION_FACTOR))
        .and_then(|n| {
            cost.string_bytes
                .checked_mul(HIR_STRING_COPY_FACTOR)
                .and_then(|strings| n.checked_add(strings))
        })
        .and_then(|n| {
            slots
                .checked_mul(maximum_identity)
                .and_then(|n| n.checked_mul(HIR_IDENTITY_COPY_FACTOR))
                .and_then(|identities| n.checked_add(identities))
        })
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    // The same live bound applies to this independent term before combining it
    // with the authored expression bundle; no synthetic AST clone owns it.
    super::checked_usage(0, bytes, "builder_bytes", active_builder_limit())
}

#[cfg(test)]
#[path = "prelude_forecast_tests.rs"]
mod tests;
