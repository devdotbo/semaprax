//! Exact flat/owned/nested conditional outcome admission in original order.
use super::*;
use crate::hir::{DeclarationKind, IdentityOrigin, ResolvedFieldDeclaration};

pub(in crate::cleanup_plan::replay::paid_type_facts) fn vector(
    index: &DeclarationIndex,
    ty: &ResolvedType,
    owned: bool,
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
    if !inventory_text::same_bytes(declaration.as_str(), crate::prelude::VEC_ID, payer)? {
        return Ok(false);
    }
    metadata::step(payer, 1)?;
    let [element] = arguments.as_slice() else {
        return Ok(false);
    };
    let probes = index.inventory_probes();
    metadata::with_work(payer, |work| {
        if owned {
            Ok(
                crate::hir::owned_leaf_collection::layout_with_work(&probes, element, work)?
                    .is_some(),
            )
        } else {
            Ok(
                crate::hir::copy_record_collection::fields_with_work(&probes, element, work)?
                    .is_some(),
            )
        }
    })
}
pub(in crate::cleanup_plan::replay::paid_type_facts) fn field(
    index: &DeclarationIndex,
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    Ok(vector(index, ty, false, payer)? || vector(index, ty, true, payer)?)
}
fn flat(
    index: &DeclarationIndex,
    ty: &ResolvedType,
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
    if !arguments.is_empty()
        || metadata::declared(index, declaration, payer)?.is_none_or(|d| {
            d.kind != DeclarationKind::Variant || d.identity_origin != IdentityOrigin::Explicit
        })
        || metadata::parameters(index, declaration, payer)?.is_none_or(|p| !p.is_empty())
    {
        return Ok(false);
    }
    let Some(cases) = metadata::cases(index, declaration, payer)? else {
        return Ok(false);
    };
    metadata::step(payer, 1)?;
    if cases.len() != 2 {
        return Ok(false);
    }
    let mut owners = 0;
    for case in cases {
        metadata::step(payer, 1)?;
        if !(1..=8).contains(&case.fields.len()) {
            return Ok(false);
        }
        for field in &case.fields {
            metadata::step(payer, 1)?;
            if vector(index, &field.ty, false, payer)? {
                owners += 1;
            } else if !crate::vec_ops::resolved_element_is_admitted(&field.ty) {
                return Ok(false);
            }
        }
    }
    metadata::step(payer, 1)?;
    Ok((1..=2).contains(&owners))
}
fn success(
    index: &DeclarationIndex,
    fields: &[ResolvedFieldDeclaration],
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    if !(1..=2).contains(&fields.len()) {
        return Ok(false);
    }
    for field in fields {
        metadata::step(payer, 1)?;
        if !vector(index, &field.ty, true, payer)? {
            return Ok(false);
        }
    }
    Ok(true)
}
fn error(fields: &[ResolvedFieldDeclaration], payer: &mut Payer<'_>) -> Result<bool, Diagnostic> {
    // One slice inspection plus the three closed scalar comparisons.
    metadata::step(payer, 4)?;
    Ok(
        matches!(fields, [code,offset,field] if code.ty == ResolvedType::I64 && offset.ty == ResolvedType::Usize && field.ty == ResolvedType::I64),
    )
}
fn owned(
    index: &DeclarationIndex,
    ty: &ResolvedType,
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
    if !arguments.is_empty()
        || metadata::parameters(index, declaration, payer)?.is_none_or(|p| !p.is_empty())
        || metadata::declared(index, declaration, payer)?.is_none_or(|d| {
            d.kind != DeclarationKind::Variant || d.identity_origin != IdentityOrigin::Explicit
        })
    {
        return Ok(false);
    }
    let Some(cases) = metadata::cases(index, declaration, payer)? else {
        return Ok(false);
    };
    metadata::step(payer, 1)?;
    if cases.len() != 2 {
        return Ok(false);
    }
    for case in cases {
        metadata::step(payer, 1)?;
        if metadata::declared(index, &case.id, payer)?
            .is_none_or(|d| d.identity_origin != IdentityOrigin::Explicit)
        {
            return Ok(false);
        }
        for field in &case.fields {
            metadata::step(payer, 1)?;
            if metadata::declared(index, &field.id, payer)?
                .is_none_or(|d| d.identity_origin != IdentityOrigin::Explicit)
            {
                return Ok(false);
            }
        }
    }
    if success(index, &cases[0].fields, payer)? && error(&cases[1].fields, payer)? {
        return Ok(true);
    }
    Ok(success(index, &cases[1].fields, payer)? && error(&cases[0].fields, payer)?)
}
pub(in crate::cleanup_plan::replay::paid_type_facts) fn admitted(
    index: &DeclarationIndex,
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    Ok(flat(index, ty, payer)?
        || owned(index, ty, payer)?
        || inventory_nested::admitted(index, ty, payer)?)
}
