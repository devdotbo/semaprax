//! Exact held metadata queries on the existing replay work authority. No
//! temporary identity string, copied directory, new quota or mutation occurs.
use super::*;
use crate::hir::renewal_probes::Work;
use crate::hir::{
    Declaration, DeclarationKind, IdentityOrigin, ResolvedFieldDeclaration,
    ResolvedTypeParameterDeclaration, ResolvedVariantCaseDeclaration,
};

pub(super) fn with_work<T>(
    payer: &mut Payer<'_>,
    query: impl FnOnce(&mut Work<'_>) -> Result<T, ()>,
) -> Result<T, Diagnostic> {
    inventory_text::current(payer)?;
    let mut first = None;
    let result = {
        let mut work = |units| {
            if first.is_some() {
                return false;
            }
            let result = inventory_text::current(payer).and_then(|()| {
                payer
                    .budget
                    .charge(payer.function, units, "inventory metadata comparison")
            });
            match result {
                Ok(()) => true,
                Err(error) => {
                    first = Some(error);
                    false
                }
            }
        };
        query(&mut work)
    };
    // A stateful read comparator can stop at an arbitrary map node after
    // refusal. Never consume that provisional result or continue comparisons.
    if let Some(error) = first {
        return Err(error);
    }
    result.map_err(|()| {
        payer
            .construction_refusal("inventory metadata work receiver failed without a typed refusal")
    })
}
pub(super) fn declared<'a>(
    index: &'a DeclarationIndex,
    key: &DeclarationId,
    payer: &mut Payer<'_>,
) -> Result<Option<&'a Declaration>, Diagnostic> {
    with_work(payer, |work| index.inventory_probes().declared(key, work))
}
pub(super) fn parameters<'a>(
    index: &'a DeclarationIndex,
    key: &DeclarationId,
    payer: &mut Payer<'_>,
) -> Result<Option<&'a Vec<ResolvedTypeParameterDeclaration>>, Diagnostic> {
    with_work(payer, |work| {
        index.inventory_probes().parameter_rows(key, work)
    })
}
pub(super) fn fields<'a>(
    index: &'a DeclarationIndex,
    key: &DeclarationId,
    payer: &mut Payer<'_>,
) -> Result<Option<&'a Vec<ResolvedFieldDeclaration>>, Diagnostic> {
    with_work(payer, |work| index.inventory_probes().field_rows(key, work))
}
pub(super) fn cases<'a>(
    index: &'a DeclarationIndex,
    key: &DeclarationId,
    payer: &mut Payer<'_>,
) -> Result<Option<&'a [ResolvedVariantCaseDeclaration]>, Diagnostic> {
    with_work(payer, |work| index.inventory_probes().cases(key, work))
}
pub(super) fn has_invariant(
    index: &DeclarationIndex,
    key: &DeclarationId,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    with_work(payer, |work| {
        index.inventory_probes().has_invariant(key, work)
    })
}
pub(super) fn step(payer: &mut Payer<'_>, units: usize) -> Result<(), Diagnostic> {
    inventory_text::current(payer)?;
    payer
        .budget
        .charge(payer.function, units, "inventory nested structural visit")
}
pub(super) fn vector(
    index: &DeclarationIndex,
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    step(payer, 1)?;
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    if !inventory_text::same_bytes(declaration.as_str(), crate::prelude::VEC_ID, payer)?
        || declared(index, declaration, payer)?.is_none_or(|d| {
            d.kind != DeclarationKind::Record || d.identity_origin != IdentityOrigin::CompilerOwned
        })
    {
        return Ok(false);
    }
    let [element] = arguments.as_slice() else {
        return Ok(false);
    };
    step(payer, 1)?;
    if crate::vec_ops::resolved_vec_element_is_admitted(element) {
        return Ok(true);
    }
    let probes = index.inventory_probes();
    with_work(payer, |work| {
        Ok(
            crate::hir::copy_record_collection::fields_with_work(&probes, element, work)?.is_some()
                || crate::hir::owned_leaf_collection::runtime_with_work(&probes, element, work)?,
        )
    })
}

// The caller must supply the complete identity key produced by its paid key
// constructor. A root spelling alone never authenticates generic arguments.
pub(super) fn cached_identity<'a>(
    index: &'a DeclarationIndex,
    key: &str,
    payer: &mut Payer<'_>,
) -> Result<Option<&'a crate::hir::TypeFacts>, Diagnostic> {
    with_work(payer, |work| {
        index.inventory_probes().cached_identity(key, work)
    })
}
pub(super) fn named_type<'a>(
    index: &'a DeclarationIndex,
    name: &str,
    payer: &mut Payer<'_>,
) -> Result<Option<&'a DeclarationId>, Diagnostic> {
    with_work(payer, |work| {
        index.inventory_probes().named_type(name, work)
    })
}
pub(super) fn case_fields<'a>(
    index: &'a DeclarationIndex,
    key: &DeclarationId,
    payer: &mut Payer<'_>,
) -> Result<Option<&'a Vec<ResolvedFieldDeclaration>>, Diagnostic> {
    with_work(payer, |work| index.inventory_probes().case_rows(key, work))
}
