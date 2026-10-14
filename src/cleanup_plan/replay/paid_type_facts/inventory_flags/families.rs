//! Complete builtin/profile predicates on the held existing-work metadata seam.
use super::*;
use crate::hir::{DeclarationKind, IdentityOrigin};

pub(super) fn typed_collection(
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
    if inventory_text::same_bytes(declaration.as_str(), crate::map_ops::MAP_ID, payer)? {
        metadata::step(payer, 2)?;
        return Ok(
            matches!(arguments.as_slice(), [key, value] if matches!(key, ResolvedType::String | ResolvedType::I64 | ResolvedType::Bool) && matches!(value, ResolvedType::String | ResolvedType::I64 | ResolvedType::I32 | ResolvedType::U8 | ResolvedType::Usize | ResolvedType::Char | ResolvedType::F32 | ResolvedType::F64 | ResolvedType::Bool)),
        );
    }
    if !inventory_text::same_bytes(declaration.as_str(), crate::map_ops::SET_ID, payer)? {
        return Ok(false);
    }
    metadata::step(payer, 1)?;
    Ok(matches!(
        arguments.as_slice(),
        [ResolvedType::String | ResolvedType::I64 | ResolvedType::Bool]
    ))
}
pub(super) fn signature(ty: &ResolvedType, payer: &mut Payer<'_>) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let ResolvedType::Function { parameters, result } = ty else {
        return Ok(false);
    };
    if parameters.len() > 8 {
        return Ok(false);
    }
    for parameter in parameters {
        metadata::step(payer, 1)?;
        if !crate::hir::function_value::scalar(parameter) {
            return Ok(false);
        }
    }
    metadata::step(payer, 1)?;
    Ok(crate::hir::function_value::scalar(result))
}
pub(super) fn resource_result(
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
    let [ResolvedType::Nominal {
        declaration: resource,
        arguments: inner,
    }, ResolvedType::I64] = arguments.as_slice()
    else {
        return Ok(false);
    };
    if !inventory_text::same_bytes(declaration.as_str(), crate::prelude::RESULT_ID, payer)?
        || !inner.is_empty()
    {
        return Ok(false);
    }
    Ok(metadata::declared(index, resource, payer)?
        .is_some_and(|d| d.kind == DeclarationKind::Resource))
}
pub(super) fn owned_byte_prelude(
    id: &DeclarationId,
    arguments: &[ResolvedType],
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    if inventory_text::same_bytes(id.as_str(), crate::prelude::OPTION_ID, payer)? {
        metadata::step(payer, 1)?;
        return Ok(matches!(arguments, [ResolvedType::Bytes]));
    }
    if !inventory_text::same_bytes(id.as_str(), crate::prelude::RESULT_ID, payer)? {
        return Ok(false);
    }
    metadata::step(payer, 2)?;
    Ok(matches!(
        arguments,
        [
            ResolvedType::Bytes,
            ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool
        ] | [
            ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool,
            ResolvedType::Bytes
        ] | [ResolvedType::Bytes, ResolvedType::Bytes]
    ))
}
fn declared_text(
    index: &DeclarationIndex,
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    if matches!(ty, ResolvedType::String) {
        return Ok(true);
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    if !arguments.is_empty()
        || metadata::declared(index, declaration, payer)?.is_none_or(|d| {
            d.kind != DeclarationKind::Record || d.identity_origin != IdentityOrigin::Explicit
        })
        || metadata::parameters(index, declaration, payer)?.is_none_or(|p| !p.is_empty())
    {
        return Ok(false);
    }
    let Some(fields) = metadata::fields(index, declaration, payer)? else {
        return Ok(false);
    };
    if !(1..=8).contains(&fields.len()) {
        return Ok(false);
    }
    let mut strings = 0usize;
    for field in fields {
        metadata::step(payer, 1)?;
        strings += usize::from(matches!(field.ty, ResolvedType::String));
    }
    if strings != 1 {
        return Ok(false);
    }
    for field in fields {
        metadata::step(payer, 1)?;
        if !matches!(field.ty, ResolvedType::String)
            && !crate::vec_ops::resolved_element_is_admitted(&field.ty)
        {
            return Ok(false);
        }
    }
    Ok(true)
}
fn owned_record(
    index: &DeclarationIndex,
    element: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let probes = index.inventory_probes();
    metadata::with_work(payer, |work| {
        crate::hir::owned_record_collection::element_with_work(&probes, element, work)
    })
}
fn leaf(
    index: &DeclarationIndex,
    element: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let probes = index.inventory_probes();
    metadata::with_work(payer, |work| {
        Ok(crate::hir::owned_leaf_collection::layout_with_work(&probes, element, work)?.is_some())
    })
}
pub(super) fn builtin(
    index: &DeclarationIndex,
    id: &DeclarationId,
    arguments: &[ResolvedType],
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    if inventory_text::same_bytes(id.as_str(), crate::stdin_stream_ops::READER_ID, payer)?
        && arguments.is_empty()
    {
        return Ok(true);
    }
    let iterator = inventory_text::same_bytes(id.as_str(), crate::iterator_ops::ITER_ID, payer)?
        || inventory_text::same_bytes(id.as_str(), crate::iterator_ops::STEP_ID, payer)?;
    if iterator {
        if let [element] = arguments {
            metadata::step(payer, 1)?;
            let probes = index.inventory_probes();
            let admitted = crate::iterator_ops::resolved_element_is_admitted(element)
                || metadata::with_work(payer, |work| {
                    crate::hir::owned_leaf_collection::runtime_with_work(&probes, element, work)
                })?;
            if admitted {
                // Retain old profile-selection inspections and their order even
                // though the selected layout spelling is never materialized.
                if !owned_record(index, element, payer)? && !leaf(index, element, payer)? {
                    metadata::step(payer, 1)?;
                    let _bytes_version = matches!(element, ResolvedType::Bytes);
                }
                return Ok(true);
            }
        }
    }
    let [element] = arguments else {
        return Ok(false);
    };
    if inventory_text::same_bytes(id.as_str(), crate::prelude::VEC_ID, payer)? {
        metadata::step(payer, 1)?;
        if crate::vec_ops::resolved_vec_element_is_admitted(element) {
            return Ok(true);
        }
        let probes = index.inventory_probes();
        if metadata::with_work(payer, |work| {
            Ok(
                crate::hir::copy_record_collection::fields_with_work(&probes, element, work)?
                    .is_some(),
            )
        })? || owned_record(index, element, payer)?
        {
            return Ok(true);
        }
        if declared_text(index, element, payer)? {
            return Ok(true);
        }
        return leaf(index, element, payer);
    }
    if !inventory_text::same_bytes(id.as_str(), crate::prelude::BOX_ID, payer)? {
        return Ok(false);
    }
    metadata::step(payer, 1)?;
    Ok(crate::box_ops::resolved_box_element_is_admitted(element))
}

pub(super) fn iterator_element(
    index: &DeclarationIndex,
    element: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    if crate::iterator_ops::resolved_element_is_admitted(element) {
        return Ok(true);
    }
    let probes = index.inventory_probes();
    metadata::with_work(payer, |work| {
        crate::hir::owned_leaf_collection::runtime_with_work(&probes, element, work)
    })
}
pub(super) fn vector_element(
    index: &DeclarationIndex,
    element: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    if crate::vec_ops::resolved_vec_element_is_admitted(element) {
        return Ok(true);
    }
    let probes = index.inventory_probes();
    // Original outer cleanup order: copy-record, owned-leaf, owned-record.
    if metadata::with_work(payer, |work| {
        Ok(crate::hir::copy_record_collection::fields_with_work(&probes, element, work)?.is_some())
    })? || leaf(index, element, payer)?
    {
        return Ok(true);
    }
    owned_record(index, element, payer)
}
