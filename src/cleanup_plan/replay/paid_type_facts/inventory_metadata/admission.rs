//! Complete native/authored-byte admission on the same held metadata payer.
//! No cache facts grant admission, and no comparison refusal becomes false.
use super::*;
fn named_is(
    index: &DeclarationIndex,
    name: &str,
    id: &DeclarationId,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    match named_type(index, name, payer)? {
        Some(found) => inventory_text::same_bytes(found.as_str(), id.as_str(), payer),
        None => Ok(false),
    }
}
pub(in crate::cleanup_plan::replay::paid_type_facts) fn native_owned(
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
    if named_is(index, "Option", declaration, payer)? {
        step(payer, 1)?;
        if arguments.as_slice() == [ResolvedType::String] {
            return Ok(true);
        }
    }
    if !named_is(index, "Result", declaration, payer)? {
        return Ok(false);
    }
    step(payer, 1)?;
    match arguments.as_slice() {
        [ResolvedType::String, ResolvedType::I64] => Ok(true),
        [ResolvedType::String, ResolvedType::Nominal {
            declaration,
            arguments,
        }] => {
            if !named_is(index, "Option", declaration, payer)? {
                return Ok(false);
            }
            step(payer, 1)?;
            Ok(arguments.as_slice() == [ResolvedType::I64])
        }
        _ => Ok(false),
    }
}
fn substituted_leaf<'a>(
    template: &'a ResolvedType,
    owner: &DeclarationId,
    arguments: &'a [ResolvedType],
    payer: &mut Payer<'_>,
) -> Result<Option<&'a ResolvedType>, Diagnostic> {
    step(payer, 1)?;
    match template {
        ResolvedType::TypeParameter {
            owner: actual,
            index,
        } => {
            if !inventory_text::same_bytes(actual.as_str(), owner.as_str(), payer)? {
                return Ok(None);
            }
            Ok(usize::try_from(*index)
                .ok()
                .and_then(|index| arguments.get(index)))
        }
        ResolvedType::Nominal { .. } | ResolvedType::Function { .. } => Ok(None),
        leaf => Ok(Some(leaf)),
    }
}
pub(in crate::cleanup_plan::replay::paid_type_facts) fn authored_byte_variant(
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
    let Some(item) = declared(index, declaration, payer)? else {
        return Ok(false);
    };
    if item.kind != DeclarationKind::Variant || item.identity_origin != IdentityOrigin::Explicit {
        return Ok(false);
    }
    let Some(parameters) = parameters(index, declaration, payer)? else {
        return Ok(false);
    };
    if arguments.len() != parameters.len() {
        return Ok(false);
    }
    for argument in arguments {
        step(payer, 1)?;
        if *argument != ResolvedType::Bytes
            && !crate::hir::type_reachability::nested_record_copy_scalar_is_admitted(argument)
        {
            return Ok(false);
        }
    }
    let Some(cases) = cases(index, declaration, payer)? else {
        return Ok(false);
    };
    let mut owned_cases = 0usize;
    for case in cases {
        step(payer, 1)?;
        let Some(fields) = case_fields(index, &case.id, payer)? else {
            return Ok(false);
        };
        let mut owns_bytes = false;
        for field in fields {
            let Some(field) = substituted_leaf(&field.ty, declaration, arguments, payer)? else {
                return Ok(false);
            };
            step(payer, 1)?;
            if *field == ResolvedType::Bytes {
                owns_bytes = true;
            } else if !crate::hir::type_reachability::nested_record_copy_scalar_is_admitted(field) {
                return Ok(false);
            }
        }
        // This count cannot exceed the already borrowed Vec's length.
        owned_cases += usize::from(owns_bytes);
    }
    Ok(owned_cases == 1
        || (owned_cases == 2
            && parameters.len() == 2
            && arguments.as_slice() == [ResolvedType::Bytes, ResolvedType::Bytes]
            && cases.len() == 2))
}
