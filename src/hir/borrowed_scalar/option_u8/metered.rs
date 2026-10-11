//! Borrowed exact metadata receiver. Each evaluated probe and byte comparison
//! debits the caller's ledger before work; no candidate layout is reconstructed.
use super::*;

fn equal<E>(a: &str, b: &str, charge: &mut impl FnMut(usize) -> Result<(), E>) -> Result<bool, E> {
    charge(1)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.bytes().zip(b.bytes()) {
        charge(1)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
fn lookup<'a, T, E>(
    table: &'a BTreeMap<DeclarationId, T>,
    key: &DeclarationId,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<Option<&'a T>, E> {
    // A borrowed ordered iterator avoids hidden key comparisons in BTreeMap::get.
    // It retains original exact key equality, including hostile nearby IDs.
    for (candidate, value) in table {
        charge(1)?;
        if equal(candidate.as_str(), key.as_str(), charge)? {
            return Ok(Some(value));
        }
    }
    Ok(None)
}
fn owner<E>(
    actual: &Option<DeclarationId>,
    expected: &DeclarationId,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    charge(1)?;
    match actual {
        Some(actual) => equal(actual.as_str(), expected.as_str(), charge),
        None => Ok(false),
    }
}
fn field<E>(
    actual: &ResolvedFieldDeclaration,
    expected: &ResolvedFieldDeclaration,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    charge(1)?;
    if !equal(actual.id.as_str(), expected.id.as_str(), charge)?
        || !equal(&actual.name, &expected.name, charge)?
        || actual.index != expected.index
        || actual.span != expected.span
    {
        return Ok(false);
    }
    match (&actual.ty, &expected.ty) {
        (
            ResolvedType::TypeParameter {
                owner: a,
                index: ai,
            },
            ResolvedType::TypeParameter {
                owner: b,
                index: bi,
            },
        ) => Ok(ai == bi && equal(a.as_str(), b.as_str(), charge)?),
        _ => Ok(false),
    }
}

pub(crate) fn authenticated<E>(
    index: &DeclarationIndex,
    ty: &ResolvedType,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    charge(1)?;
    let ResolvedType::Nominal {
        declaration: option,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    if !equal(option.as_str(), crate::prelude::OPTION_ID, charge)?
        || arguments.as_slice() != [ResolvedType::U8]
    {
        return Ok(false);
    }
    let Some(declaration) = lookup(&index.declarations, option, charge)? else {
        return Ok(false);
    };
    charge(1)?;
    if declaration.kind != DeclarationKind::Variant
        || !equal(&declaration.name, "Option", charge)?
        || declaration.identity_origin != IdentityOrigin::CompilerOwned
        || declaration.owner.is_some()
    {
        return Ok(false);
    }
    let Some(parameters) = lookup(&index.type_parameters, option, charge)? else {
        return Ok(false);
    };
    charge(1)?;
    if parameters.len() != 1
        || !equal(&parameters[0].name, "T", charge)?
        || parameters[0].index != 0
    {
        return Ok(false);
    }
    let Some(cases) = lookup(&index.variant_cases, option, charge)? else {
        return Ok(false);
    };
    charge(1)?;
    if cases.len() != 2 {
        return Ok(false);
    }
    for (case, id, name, ordinal, fields) in [
        (&cases[0], crate::prelude::OPTION_NONE_ID, "None", 0, 0),
        (&cases[1], crate::prelude::OPTION_SOME_ID, "Some", 1, 1),
    ] {
        charge(1)?;
        if !equal(case.id.as_str(), id, charge)?
            || !equal(&case.name, name, charge)?
            || case.index != ordinal
            || case.fields.len() != fields
        {
            return Ok(false);
        }
    }
    for case in cases {
        let Some(declaration) = lookup(&index.declarations, &case.id, charge)? else {
            return Ok(false);
        };
        charge(1)?;
        if declaration.kind != DeclarationKind::VariantCase
            || !equal(&declaration.name, &case.name, charge)?
            || declaration.identity_origin != IdentityOrigin::CompilerOwned
            || !owner(&declaration.owner, option, charge)?
        {
            return Ok(false);
        }
    }
    let value = &cases[1].fields[0];
    let Some(declaration) = lookup(&index.declarations, &value.id, charge)? else {
        return Ok(false);
    };
    charge(1)?;
    if declaration.kind != DeclarationKind::CaseField
        || !equal(&declaration.name, "value", charge)?
        || declaration.identity_origin != IdentityOrigin::CompilerOwned
        || !owner(&declaration.owner, &cases[1].id, charge)?
        || !equal(
            value.id.as_str(),
            crate::prelude::OPTION_SOME_VALUE_ID,
            charge,
        )?
        || !equal(&value.name, "value", charge)?
        || value.index != 0
    {
        return Ok(false);
    }
    charge(1)?;
    let ResolvedType::TypeParameter {
        owner: parameter_owner,
        index: 0,
    } = &value.ty
    else {
        return Ok(false);
    };
    if !equal(parameter_owner.as_str(), option.as_str(), charge)? {
        return Ok(false);
    }
    let Some(fields) = lookup(&index.case_fields, &cases[1].id, charge)? else {
        return Ok(false);
    };
    charge(1)?;
    if fields.len() != 1 || !field(&fields[0], value, charge)? {
        return Ok(false);
    }
    let Some(fields) = lookup(&index.case_fields, &cases[0].id, charge)? else {
        return Ok(false);
    };
    charge(1)?;
    Ok(fields.is_empty())
}

/// Exact single-case pattern receiver; no source spelling or general variant.
#[cfg(test)]
pub(crate) fn pattern<E>(
    pattern: &ResolvedMatchPattern,
    charge: &mut impl FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    charge(1)?;
    let ResolvedMatchPattern::Variant {
        variant,
        case,
        fields,
    } = pattern
    else {
        return Ok(false);
    };
    if !equal(variant.as_str(), crate::prelude::OPTION_ID, charge)? {
        return Ok(false);
    }
    if equal(case.as_str(), crate::prelude::OPTION_NONE_ID, charge)? {
        return Ok(fields.is_empty());
    }
    if !equal(case.as_str(), crate::prelude::OPTION_SOME_ID, charge)? {
        return Ok(false);
    }
    charge(1)?;
    let [field] = fields.as_slice() else {
        return Ok(false);
    };
    Ok(equal(
        field.field.as_str(),
        crate::prelude::OPTION_SOME_VALUE_ID,
        charge,
    )? && field.binding.ty == ResolvedType::U8
        && field.binding.ownership == OwnershipMode::Value)
}
