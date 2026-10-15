//! Borrowed exact counterparts of discarded admission-only owned type trees.
//! These comparisons neither trust TypeFacts nor authenticate a new profile.
use crate::hir::{DeclarationId, DeclarationIndex, DeclarationKind, IdentityOrigin, ResolvedType};

pub(super) fn native_owned(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    // value_type's exact current display-name lookup is retained, including
    // missing/hostile mappings. Only the discarded Vec/nominal tree is removed.
    if index.type_id("Option") == Some(declaration)
        && arguments.as_slice() == [ResolvedType::String]
    {
        return true;
    }
    if index.type_id("Result") != Some(declaration) {
        return false;
    }
    match arguments.as_slice() {
        [ResolvedType::String, ResolvedType::I64] => true,
        [ResolvedType::String, ResolvedType::Nominal {
            declaration,
            arguments,
        }] => {
            index.type_id("Option") == Some(declaration)
                && arguments.as_slice() == [ResolvedType::I64]
        }
        _ => false,
    }
}

fn substituted_leaf<'a>(
    template: &'a ResolvedType,
    owner: &DeclarationId,
    arguments: &'a [ResolvedType],
) -> Option<&'a ResolvedType> {
    match template {
        ResolvedType::TypeParameter {
            owner: actual,
            index,
        } => {
            if actual != owner {
                return None;
            }
            arguments.get(usize::try_from(*index).ok()?)
        }
        // Substitution never changes a compound root into a scalar/Bytes root.
        // Even a malformed compound's complete old substitution can only be
        // Err or a compound, both refused by the original leaf predicate.
        ResolvedType::Nominal { .. } | ResolvedType::Function { .. } => None,
        leaf => Some(leaf),
    }
}

pub(super) fn authored_byte_variant(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return false;
    };
    let Some(item) = index.declaration(declaration) else {
        return false;
    };
    if item.kind != DeclarationKind::Variant || item.identity_origin != IdentityOrigin::Explicit {
        return false;
    }
    let Some(parameters) = index.type_parameters(declaration) else {
        return false;
    };
    if arguments.len() != parameters.len()
        || arguments.iter().any(|argument| {
            *argument != ResolvedType::Bytes
                && !crate::hir::type_reachability::nested_record_copy_scalar_is_admitted(argument)
        })
    {
        return false;
    }
    let Some(cases) = index.variant_cases(declaration) else {
        return false;
    };
    let mut owned_cases = 0usize;
    for case in cases {
        let Some(fields) = index.case_fields(&case.id) else {
            return false;
        };
        let mut owns_bytes = false;
        for field in fields {
            let Some(field) = substituted_leaf(&field.ty, declaration, arguments) else {
                return false;
            };
            if *field == ResolvedType::Bytes {
                owns_bytes = true;
            } else if !crate::hir::type_reachability::nested_record_copy_scalar_is_admitted(field) {
                return false;
            }
        }
        owned_cases += usize::from(owns_bytes);
    }
    owned_cases == 1
        || (owned_cases == 2
            && parameters.len() == 2
            && arguments.as_slice() == [ResolvedType::Bytes, ResolvedType::Bytes]
            && cases.len() == 2)
}
