//! OPT814 summary step-shape validation on the held replay work authority.
//! Borrowed structural comparisons only: no expected tree, Vec or owned text.
use super::*;
use crate::hir::CompilerDeclaration;
use crate::iterator_ops::{DONE_ID, ITEM_ID, ITER_ID, REST_ID, YIELD_ID};

macro_rules! hold {
    ($e:expr) => {
        if !$e? {
            return Ok(false);
        }
    };
}

fn same(payer: &mut Payer<'_>, actual: &u32, expected: u32) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    Ok(*actual == expected)
}

// Exactly TypeParameter { owner: IterStep, index: 0 }, full owner id compared.
fn parameter(
    ty: &ResolvedType,
    owner: &DeclarationId,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let ResolvedType::TypeParameter {
        owner: actual,
        index,
    } = ty
    else {
        return Ok(false);
    };
    hold!(inventory_text::owner_matches(actual, owner, payer));
    same(payer, index, 0)
}

// Exactly Nominal { ITER_ID, [that parameter] }: the whole argument slice.
fn iter_of_parameter(
    ty: &ResolvedType,
    owner: &DeclarationId,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    hold!(inventory_text::same_bytes(
        declaration.as_str(),
        ITER_ID,
        payer
    ));
    metadata::step(payer, 1)?;
    let [argument] = arguments.as_slice() else {
        return Ok(false);
    };
    parameter(argument, owner, payer)
}

// These exact original root inspections also use paid byte comparisons;
// an ordinary is_step/element wrapper would leave dynamic identities unpaid.
pub(in crate::cleanup_plan::replay::paid_type_facts) fn element<'a>(
    ty: &'a ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<Option<&'a ResolvedType>, Diagnostic> {
    metadata::step(payer, 1)?;
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(None);
    };
    if !inventory_text::same_bytes(declaration.as_str(), crate::iterator_ops::ITER_ID, payer)?
        && !inventory_text::same_bytes(declaration.as_str(), crate::iterator_ops::STEP_ID, payer)?
    {
        return Ok(None);
    }
    metadata::step(payer, 1)?;
    Ok(match arguments.as_slice() {
        [element] => Some(element),
        _ => None,
    })
}
pub(in crate::cleanup_plan::replay::paid_type_facts) fn is_step(
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let ResolvedType::Nominal { declaration, .. } = ty else {
        return Ok(false);
    };
    if !inventory_text::same_bytes(declaration.as_str(), crate::iterator_ops::STEP_ID, payer)? {
        return Ok(false);
    }
    Ok(element(ty, payer)?.is_some())
}
pub(in crate::cleanup_plan::replay::paid_type_facts) fn step_shape(
    index: &DeclarationIndex,
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    inventory_text::current(payer)?;
    metadata::step(payer, 1)?;
    if !is_step(ty, payer)? {
        return Ok(false);
    }
    metadata::step(payer, 1)?;
    let Some(element) = element(ty, payer)? else {
        return Ok(false);
    };
    hold!(inventory_flags::families::iterator_element(
        index, element, payer
    ));
    let owner = DeclarationId::compiler(CompilerDeclaration::IterStep);
    let Some(cases) = metadata::cases(index, &owner, payer)? else {
        return Ok(false);
    };
    metadata::step(payer, 1)?;
    let [done, yielded] = cases else {
        return Ok(false);
    };
    hold!(inventory_text::same_bytes(done.id.as_str(), DONE_ID, payer));
    hold!(inventory_text::same_bytes(&done.name, "Done", payer));
    hold!(same(payer, &done.index, 0));
    metadata::step(payer, 1)?;
    if !done.fields.is_empty() {
        return Ok(false);
    }
    hold!(inventory_text::same_bytes(
        yielded.id.as_str(),
        YIELD_ID,
        payer
    ));
    hold!(inventory_text::same_bytes(&yielded.name, "Yield", payer));
    hold!(same(payer, &yielded.index, 1));
    metadata::step(payer, 1)?;
    let [item, rest] = yielded.fields.as_slice() else {
        return Ok(false);
    };
    hold!(inventory_text::same_bytes(item.id.as_str(), ITEM_ID, payer));
    hold!(inventory_text::same_bytes(&item.name, "item", payer));
    hold!(same(payer, &item.index, 0));
    hold!(parameter(&item.ty, &owner, payer));
    hold!(inventory_text::same_bytes(rest.id.as_str(), REST_ID, payer));
    hold!(inventory_text::same_bytes(&rest.name, "rest", payer));
    hold!(same(payer, &rest.index, 1));
    iter_of_parameter(&rest.ty, &owner, payer)
}
