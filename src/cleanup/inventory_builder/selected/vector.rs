//! Borrowed current-declaration Vec classification. No generic substitution,
//! inferred layout, nested element or attached cleanup fact grants authority.
use super::*;

pub(crate) fn element<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    ty: &'a ResolvedType,
) -> Result<Option<&'a ResolvedType>, Diagnostic> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(None);
    };
    if declaration.as_str() != crate::prelude::VEC_ID {
        return Ok(None);
    }
    authenticate(program, declaration)?;
    let [element] = arguments.as_slice() else {
        return Err(cleanup_error(
            "selected Vec has invalid type argument count",
        ));
    };
    if crate::vec_ops::resolved_vec_element_is_admitted(element) || *element == ResolvedType::String
    {
        return Ok(Some(element));
    }
    if matches!(element, ResolvedType::Nominal { arguments, .. } if arguments.is_empty()) {
        flat_record::fields(program, element)?;
        if crate::hir::copy_record_collection::admitted(program.declarations(), element)
            || crate::hir::owned_leaf_collection::runtime_element(program.declarations(), element)
        {
            return Ok(Some(element));
        }
    }
    Err(cleanup_error(
        "selected Vec element is outside the current bounded flat family",
    ))
}
pub(crate) fn authenticate(
    program: &dyn crate::cleanup::ReplayContext,
    id: &DeclarationId,
) -> Result<(), Diagnostic> {
    if program.type_count() > 32 {
        return Err(cleanup_error(
            "selected Vec current directory exceeds its bounded tranche",
        ));
    }
    let mut current = None;
    for ordinal in 0..program.type_count() {
        let item = program
            .type_at(ordinal)
            .ok_or_else(|| cleanup_error("selected Vec current directory is incomplete"))?;
        if &item.id == id {
            if current.replace(item).is_some() {
                return Err(cleanup_error(
                    "selected Vec current directory repeats its identity",
                ));
            }
        }
    }
    let current = current
        .ok_or_else(|| cleanup_error("selected Vec declaration is absent from current types"))?;
    let index = program.declarations();
    let fail =
        || cleanup_error("selected Vec declaration is not the exact compiler-owned collection");
    let declaration = index.declaration(id).ok_or_else(fail)?;
    let parameters = index.type_parameters(id).ok_or_else(fail)?;
    if id.as_str() != crate::prelude::VEC_ID
        || declaration.kind != crate::hir::DeclarationKind::Record
        || declaration.identity_origin != crate::hir::IdentityOrigin::CompilerOwned
        || declaration.owner.is_some()
        || declaration.name != "Vec"
        || index.record_fields(id) != Some(&[])
        || current.name != "Vec"
        || current.type_parameters.as_slice() != parameters
        || !matches!(&current.kind,ResolvedTypeDeclarationKind::Record{fields} if fields.is_empty())
        || parameters.len() != 1
        || parameters[0].name != "T"
        || parameters[0].index != 0
    {
        return Err(fail());
    }
    Ok(())
}
