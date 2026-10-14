//! Complete ordinary variant record/leaf lifecycle authority, in original order.
use super::*;
use crate::hir::ResolvedTypeDeclarationKind;

pub(in crate::cleanup_plan::replay::paid_type_facts) fn primitive(
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<Option<&'static str>, Diagnostic> {
    metadata::step(payer, 1)?;
    let id = match ty {
        ResolvedType::OnceFunction => Some(crate::hir::CompilerDeclaration::OnceDrop.text()),
        ResolvedType::OnceFunctionI64 => {
            Some(crate::hir::CompilerDeclaration::OnceMixedDrop.text())
        }
        ResolvedType::OnceFunctionI64Pair => {
            Some(crate::hir::CompilerDeclaration::OncePairDrop.text())
        }
        ResolvedType::Bytes => Some(crate::cleanup::BYTES_DROP_LIFECYCLE_ID),
        ResolvedType::String => Some(crate::cleanup::STRING_DROP_LIFECYCLE_ID),
        ResolvedType::StringMap => Some(crate::hir::CompilerDeclaration::StringMapDrop.text()),
        _ => None,
    };
    if id.is_some() {
        return Ok(id);
    }
    if inventory_flags::families::typed_collection(ty, payer)? {
        return Ok(Some(crate::hir::CompilerDeclaration::CollectionDrop.text()));
    }
    Ok(reader(ty, payer)?.then_some(crate::hir::CompilerDeclaration::StdinReaderDrop.text()))
}
fn reader(ty: &ResolvedType, payer: &mut Payer<'_>) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    Ok(inventory_text::same_bytes(
        declaration.as_str(),
        crate::stdin_stream_ops::READER_ID,
        payer,
    )? && arguments.is_empty())
}
pub(in crate::cleanup_plan::replay::paid_type_facts) fn is_iter(
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let ResolvedType::Nominal { declaration, .. } = ty else {
        return Ok(false);
    };
    Ok(
        inventory_text::same_bytes(declaration.as_str(), crate::iterator_ops::ITER_ID, payer)?
            && step::element(ty, payer)?.is_some(),
    )
}
fn same_element(
    a: Option<&ResolvedType>,
    b: Option<&ResolvedType>,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    match (a, b) {
        (Some(a), Some(b)) => equal::same(a, b, payer),
        (None, None) => Ok(true),
        _ => Ok(false),
    }
}
pub(in crate::cleanup_plan::replay::paid_type_facts) fn record(
    program: &dyn crate::cleanup::ReplayContext,
    container: &ResolvedType,
    case: &DeclarationId,
    field: &DeclarationId,
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    if inventory_nested::record_field(program.declarations(), container, case, field, ty, payer)? {
        return Ok(true);
    }
    if !step::step_shape(program.declarations(), container, payer)?
        || !inventory_text::same_bytes(case.as_str(), crate::iterator_ops::YIELD_ID, payer)?
        || !inventory_text::same_bytes(field.as_str(), crate::iterator_ops::ITEM_ID, payer)?
        || !same_element(step::element(container, payer)?, Some(ty), payer)?
    {
        return Ok(false);
    }
    metadata::step(payer, 1)?;
    if !matches!(ty, ResolvedType::Nominal { .. }) {
        return Ok(false);
    }
    let probes = program.declarations().inventory_probes();
    metadata::with_work(payer, |work| {
        crate::hir::owned_leaf_collection::runtime_with_work(&probes, ty, work)
    })
}
pub(in crate::cleanup_plan::replay::paid_type_facts) fn leaf<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    container: &ResolvedType,
    case: &DeclarationId,
    field: &DeclarationId,
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<Option<&'a str>, Diagnostic> {
    if let Some(id) = primitive(ty, payer)? {
        return Ok(Some(id));
    }
    if outcome::admitted(program.declarations(), container, payer)?
        && outcome::field(program.declarations(), ty, payer)?
    {
        metadata::step(payer, 1)?;
        if let ResolvedType::Nominal {
            declaration,
            arguments,
        } = container
        {
            if arguments.is_empty() {
                if let Some(cases) = metadata::cases(program.declarations(), declaration, payer)? {
                    for candidate in cases {
                        metadata::step(payer, 1)?;
                        if !inventory_text::owner_matches(&candidate.id, case, payer)? {
                            continue;
                        }
                        for candidate in &candidate.fields {
                            metadata::step(payer, 1)?;
                            if inventory_text::owner_matches(&candidate.id, field, payer)?
                                && equal::same(&candidate.ty, ty, payer)?
                            {
                                return Ok(Some(crate::cleanup::VEC_DROP_LIFECYCLE_ID));
                            }
                        }
                    }
                }
            }
        }
    }
    if native::result(program, container, payer)?
        && inventory_text::same_bytes(case.as_str(), crate::prelude::RESULT_OK_ID, payer)?
        && inventory_text::same_bytes(field.as_str(), crate::prelude::RESULT_OK_VALUE_ID, payer)?
    {
        let ResolvedType::Nominal { arguments, .. } = container else {
            return Ok(None);
        };
        if !same_element(arguments.first(), Some(ty), payer)? {
            return Ok(None);
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return Ok(None);
        };
        metadata::step(payer, 1)?;
        if !arguments.is_empty() {
            return Ok(None);
        }
        for item in program.types() {
            metadata::step(payer, 1)?;
            if let ResolvedTypeDeclarationKind::Resource { drop } = &item.kind {
                if inventory_text::owner_matches(&item.id, declaration, payer)? {
                    return Ok(Some(drop.id.as_str()));
                }
            }
        }
        return Ok(None);
    }
    if step::is_step(container, payer)?
        && inventory_text::same_bytes(case.as_str(), crate::iterator_ops::YIELD_ID, payer)?
        && inventory_text::same_bytes(field.as_str(), crate::iterator_ops::REST_ID, payer)?
        && is_iter(ty, payer)?
    {
        let a = step::element(container, payer)?;
        let b = step::element(ty, payer)?;
        if same_element(a, b, payer)? {
            return Ok(Some(crate::cleanup::ITER_DROP_LIFECYCLE_ID));
        }
    }
    Ok(None)
}
