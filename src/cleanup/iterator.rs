//! Exact conditional iterator and selected native Result ownership.
use crate::hir::{CompilerDeclaration, DeclarationId, ResolvedType};

pub(crate) fn variant_record_field(
    program: &dyn crate::cleanup::ReplayContext,
    container: &ResolvedType,
    case: &DeclarationId,
    field: &DeclarationId,
    ty: &ResolvedType,
) -> bool {
    crate::hir::collection_outcome::nested::record_field(
        program.declarations(),
        container,
        case,
        field,
        ty,
    ) || (crate::iterator_ops::step_shape(program.declarations(), container)
        && case.as_str() == crate::iterator_ops::YIELD_ID
        && field.as_str() == crate::iterator_ops::ITEM_ID
        && crate::iterator_ops::element(container) == Some(ty)
        && matches!(ty, ResolvedType::Nominal { .. })
        && crate::hir::owned_leaf_collection::runtime_element(program.declarations(), ty))
}

pub(crate) fn variant_leaf_lifecycle<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    container: &ResolvedType,
    case: &DeclarationId,
    field: &DeclarationId,
    ty: &ResolvedType,
) -> Option<&'a str> {
    if let Some(lifecycle) = primitive_leaf_lifecycle(ty) {
        return Some(lifecycle);
    }
    if crate::hir::collection_outcome::runtime_admitted(program.declarations(), container)
        && crate::hir::collection_outcome::field_admitted(program.declarations(), ty)
        && matches!(container, ResolvedType::Nominal { declaration, arguments }
            if arguments.is_empty()
                && program.declarations().variant_cases(declaration).is_some_and(|cases|
                    cases.iter().any(|candidate| &candidate.id == case
                        && candidate.fields.iter().any(|candidate|
                            &candidate.id == field && &candidate.ty == ty))))
    {
        return Some(super::VEC_DROP_LIFECYCLE_ID);
    }
    if crate::hir::admitted_ri06_regex_result(program, container)
        && case.as_str() == crate::prelude::RESULT_OK_ID
        && field.as_str() == crate::prelude::RESULT_OK_VALUE_ID
    {
        let ResolvedType::Nominal { arguments, .. } = container else {
            return None;
        };
        if arguments.first() != Some(ty) {
            return None;
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return None;
        };
        if !arguments.is_empty() {
            return None;
        }
        return program.types().iter().find_map(|item| match &item.kind {
            crate::hir::ResolvedTypeDeclarationKind::Resource { drop }
                if &item.id == declaration =>
            {
                Some(drop.id.as_str())
            }
            _ => None,
        });
    }
    (crate::iterator_ops::is_step(container)
        && case.as_str() == crate::iterator_ops::YIELD_ID
        && field.as_str() == crate::iterator_ops::REST_ID
        && crate::iterator_ops::is_iter(ty)
        && crate::iterator_ops::element(container) == crate::iterator_ops::element(ty))
    .then_some(super::ITER_DROP_LIFECYCLE_ID)
}

/// Compiler-owned primitive cleanup identities, independently derived from type.
pub(crate) fn primitive_leaf_lifecycle(ty: &ResolvedType) -> Option<&'static str> {
    primitive_leaf_lifecycle_identity(ty).map(CompilerDeclaration::text)
}

/// The same closed mapping as fixed compiler identities, which allocate nothing.
pub(crate) fn primitive_leaf_lifecycle_identity(ty: &ResolvedType) -> Option<CompilerDeclaration> {
    match ty {
        ResolvedType::OnceFunction => Some(CompilerDeclaration::OnceDrop),
        ResolvedType::OnceFunctionI64 => Some(CompilerDeclaration::OnceMixedDrop),
        ResolvedType::OnceFunctionI64Pair => Some(CompilerDeclaration::OncePairDrop),
        ResolvedType::Bytes => Some(CompilerDeclaration::BytesDrop),
        ResolvedType::String => Some(CompilerDeclaration::StringDrop),
        ResolvedType::StringMap => Some(CompilerDeclaration::StringMapDrop),
        ty if crate::map_ops::is_typed_collection(ty) => Some(CompilerDeclaration::CollectionDrop),
        ty if crate::stdin_stream_ops::is_reader(ty) => Some(CompilerDeclaration::StdinReaderDrop),
        _ => None,
    }
}
