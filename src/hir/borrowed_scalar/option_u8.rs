//! Exact compiler-owned Copy Option<u8>; no authored or general nominal layout.
use super::super::*;
pub(crate) fn is_type(ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments }
        if declaration.as_str() == crate::prelude::OPTION_ID
            && arguments.as_slice() == [ResolvedType::U8])
}
pub(crate) fn pattern(pattern: &ResolvedMatchPattern) -> bool {
    let ResolvedMatchPattern::Variant {
        variant,
        case,
        fields,
    } = pattern
    else {
        return false;
    };
    if variant.as_str() != crate::prelude::OPTION_ID {
        return false;
    }
    match case.as_str() {
        crate::prelude::OPTION_NONE_ID => fields.is_empty(),
        crate::prelude::OPTION_SOME_ID => matches!(fields.as_slice(), [field]
            if field.field.as_str() == crate::prelude::OPTION_SOME_VALUE_ID
            && field.binding.ty == ResolvedType::U8
            && field.binding.ownership == OwnershipMode::Value),
        _ => false,
    }
}
pub(crate) fn authenticated(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    let ResolvedType::Nominal {
        declaration: owner, ..
    } = ty
    else {
        return false;
    };
    if !is_type(ty) {
        return false;
    }
    let Some(declaration) = index.declaration(owner) else {
        return false;
    };
    if declaration.kind != DeclarationKind::Variant
        || declaration.name != "Option"
        || declaration.identity_origin != IdentityOrigin::CompilerOwned
        || declaration.owner.is_some()
    {
        return false;
    }
    if !index.type_parameters(owner).is_some_and(|parameters|
        matches!(parameters, [parameter] if parameter.name == "T" && parameter.index == 0)) { return false; }
    let Some(cases) = index.variant_cases(owner) else {
        return false;
    };
    if cases.len() != 2
        || cases[0].id.as_str() != crate::prelude::OPTION_NONE_ID
        || cases[0].name != "None"
        || cases[0].index != 0
        || !cases[0].fields.is_empty()
        || cases[1].id.as_str() != crate::prelude::OPTION_SOME_ID
        || cases[1].name != "Some"
        || cases[1].index != 1
        || cases[1].fields.len() != 1
    {
        return false;
    }
    for case in cases {
        let Some(declaration) = index.declaration(&case.id) else {
            return false;
        };
        if declaration.kind != DeclarationKind::VariantCase
            || declaration.name != case.name
            || declaration.identity_origin != IdentityOrigin::CompilerOwned
            || declaration.owner.as_ref() != Some(owner)
        {
            return false;
        }
    }
    let field = &cases[1].fields[0];
    let Some(declaration) = index.declaration(&field.id) else {
        return false;
    };
    declaration.kind == DeclarationKind::CaseField
        && declaration.name == "value"
        && declaration.identity_origin == IdentityOrigin::CompilerOwned
        && declaration.owner.as_ref() == Some(&cases[1].id)
        && field.id.as_str() == crate::prelude::OPTION_SOME_VALUE_ID
        && field.name == "value"
        && field.index == 0
        && matches!(&field.ty, ResolvedType::TypeParameter { owner: parameter_owner, index: 0 }
            if parameter_owner == owner)
        && index.case_fields(&cases[1].id) == Some(cases[1].fields.as_slice())
        && index
            .case_fields(&cases[0].id)
            .is_some_and(|fields| fields.is_empty())
}

// Receiving schema/inventory lookup work is not yet closed over the selected
// session. Keep production admission closed until that owner is implemented;
// the exact predicates and reconstruction can be exercised independently.
pub(crate) fn selected_profile_ready() -> bool {
    false
}
#[cfg(test)]
mod tests;
