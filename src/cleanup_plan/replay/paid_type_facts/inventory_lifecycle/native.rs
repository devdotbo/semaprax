//! Complete selected Url/Regex resource-result bool authority, paid read-only.
//! The original native_owner and url_view receivers remain independent. Import
//! scans retain flattening order and the exact first qualifying constructor.
use super::*;
use crate::hir::{
    OwnershipMode, ResolvedImport, ResolvedImportFailure, ResolvedImportResultKind,
    ResolvedInterface, ResolvedResourceDropKind, ResolvedTypeDeclaration,
    ResolvedTypeDeclarationKind,
};

fn find<'a>(
    interfaces: &'a [ResolvedInterface],
    payer: &mut Payer<'_>,
    mut predicate: impl FnMut(&'a ResolvedImport, &mut Payer<'_>) -> Result<bool, Diagnostic>,
) -> Result<Option<&'a ResolvedImport>, Diagnostic> {
    for interface in interfaces {
        metadata::step(payer, 1)?;
        for import in &interface.imports {
            // Each predicate inspects at most ten fixed scalar field groups in
            // addition to its separately charged dynamic strings/type identity.
            metadata::step(payer, 10)?;
            if predicate(import, payer)? {
                return Ok(Some(import));
            }
        }
    }
    Ok(None)
}
fn optional(
    value: Option<&str>,
    expected: &str,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    match value {
        Some(value) => inventory_text::same_bytes(value, expected, payer),
        None => Ok(false),
    }
}
fn digest(a: Option<&str>, b: Option<&str>, payer: &mut Payer<'_>) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    match (a, b) {
        (Some(a), Some(b)) => inventory_text::same_bytes(a, b, payer),
        (None, None) => Ok(true),
        _ => Ok(false),
    }
}
fn resource_type(
    ty: &ResolvedType,
    id: &DeclarationId,
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
    if !inventory_text::owner_matches(declaration, id, payer)? {
        return Ok(false);
    }
    metadata::step(payer, 1)?;
    Ok(arguments.is_empty())
}
fn result_resource(
    import: &ResolvedImport,
    id: &DeclarationId,
    borrowed: bool,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let actual = match (&import.result.kind, borrowed) {
        (ResolvedImportResultKind::OwnedResultResourceI64 { resource }, false)
        | (ResolvedImportResultKind::BorrowedStr { resource }, true) => resource,
        _ => return Ok(false),
    };
    inventory_text::owner_matches(actual, id, payer)
}
fn destructor(
    declaration: &ResolvedTypeDeclaration,
    interfaces: &[ResolvedInterface],
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind else {
        return Ok(false);
    };
    let ResolvedResourceDropKind::Imported {
        import: finalizer,
        import_key,
    } = &drop.kind
    else {
        return Ok(false);
    };
    Ok(find(interfaces, payer, |import, payer| {
        if !inventory_text::owner_matches(&import.id, finalizer, payer)?
            || !inventory_text::same_bytes(&import.import_key, import_key, payer)?
            || import.native_rust
            || import.index_selected
            || !import.effects.is_empty()
            || import.failure != ResolvedImportFailure::Infallible
            || import.result.kind != ResolvedImportResultKind::Unit
        {
            return Ok(false);
        }
        let [parameter] = import.parameters.as_slice() else {
            return Ok(false);
        };
        Ok(resource_type(&parameter.ty, &declaration.id, payer)?
            && parameter.ownership == OwnershipMode::Own
            && parameter.consumes_on_failure)
    })?
    .is_some())
}
fn constructor<'a>(
    declaration: &ResolvedTypeDeclaration,
    interfaces: &'a [ResolvedInterface],
    url: bool,
    payer: &mut Payer<'_>,
) -> Result<Option<&'a ResolvedImport>, Diagnostic> {
    find(interfaces, payer, |import, payer| {
        if !import.native_rust || !import.index_selected {
            return Ok(false);
        }
        if url {
            if !optional(import.rust_path.as_deref(), "url_alias::Url::parse", payer)?
                || import.selected_receiver.is_some()
                || !import
                    .selected_index_digest
                    .as_deref()
                    .is_some_and(|d| !d.is_empty())
            {
                return Ok(false);
            }
        } else if !import
            .selected_index_digest
            .as_deref()
            .is_some_and(|d| !d.is_empty())
            || import.selected_receiver.is_some()
            || !optional(
                import.rust_path.as_deref(),
                "regex_alias::Regex::new",
                payer,
            )?
        {
            return Ok(false);
        }
        if !result_resource(import, &declaration.id, false, payer)? {
            return Ok(false);
        }
        Ok(
            matches!(import.parameters.as_slice(), [p] if matches!(p.ty, ResolvedType::String) && p.ownership == OwnershipMode::Borrow),
        )
    })
}
fn method(
    declaration: &ResolvedTypeDeclaration,
    interfaces: &[ResolvedInterface],
    constructor: &ResolvedImport,
    url: bool,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    Ok(find(interfaces, payer, |import, payer| {
        if !import.native_rust || !import.index_selected {
            return Ok(false);
        }
        if url {
            if !optional(import.rust_path.as_deref(), "url_alias::Url::as_str", payer)?
                || !optional(import.selected_receiver.as_deref(), "shared", payer)?
                || !digest(
                    import.selected_index_digest.as_deref(),
                    constructor.selected_index_digest.as_deref(),
                    payer,
                )?
                || !result_resource(import, &declaration.id, true, payer)?
            {
                return Ok(false);
            }
            let [receiver] = import.parameters.as_slice() else {
                return Ok(false);
            };
            return Ok(resource_type(&receiver.ty, &declaration.id, payer)?
                && receiver.ownership == OwnershipMode::Borrow);
        }
        if !digest(
            import.selected_index_digest.as_deref(),
            constructor.selected_index_digest.as_deref(),
            payer,
        )? || !optional(import.selected_receiver.as_deref(), "shared", payer)?
            || !optional(
                import.rust_path.as_deref(),
                "regex_alias::Regex::is_match",
                payer,
            )?
            || import.result.kind != ResolvedImportResultKind::Bool
        {
            return Ok(false);
        }
        let [receiver, text] = import.parameters.as_slice() else {
            return Ok(false);
        };
        Ok(resource_type(&receiver.ty, &declaration.id, payer)?
            && receiver.ownership == OwnershipMode::Borrow
            && matches!(text.ty, ResolvedType::String)
            && text.ownership == OwnershipMode::Borrow)
    })?
    .is_some())
}
fn admitted(
    declaration: &ResolvedTypeDeclaration,
    interfaces: &[ResolvedInterface],
    url: bool,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let ResolvedTypeDeclarationKind::Resource { drop } = &declaration.kind else {
        return Ok(false);
    };
    let ResolvedResourceDropKind::Imported { .. } = &drop.kind else {
        return Ok(false);
    };
    if url {
        if !inventory_text::same_bytes(&declaration.name, "Url", payer)?
            || !declaration.type_parameters.is_empty()
        {
            return Ok(false);
        }
    } else if !declaration.type_parameters.is_empty()
        || !inventory_text::same_bytes(&declaration.name, "Regex", payer)?
    {
        return Ok(false);
    }
    let destructor = destructor(declaration, interfaces, payer)?;
    let Some(constructor) = constructor(declaration, interfaces, url, payer)? else {
        return Ok(false);
    };
    // Url uses destructor && method (short-circuit); Regex computes method
    // regardless of destructor before returning destructor && method.
    if url && !destructor {
        return Ok(false);
    }
    let method = method(declaration, interfaces, constructor, url, payer)?;
    Ok(destructor && method)
}
pub(in crate::cleanup_plan::replay::paid_type_facts) fn result(
    program: &dyn crate::cleanup::ReplayContext,
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    inventory_text::current(payer)?;
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
        arguments: resource_arguments,
    }, ResolvedType::I64] = arguments.as_slice()
    else {
        return Ok(false);
    };
    if !inventory_text::same_bytes(declaration.as_str(), crate::prelude::RESULT_ID, payer)?
        || !resource_arguments.is_empty()
    {
        return Ok(false);
    }
    for item in program.types() {
        metadata::step(payer, 1)?;
        if inventory_text::owner_matches(&item.id, resource, payer)?
            && (admitted(item, program.interfaces(), true, payer)?
                || admitted(item, program.interfaces(), false, payer)?)
        {
            return Ok(true);
        }
    }
    Ok(false)
}
