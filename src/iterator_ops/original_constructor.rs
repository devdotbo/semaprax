use super::*;

pub(crate) fn step_shape(index: &crate::hir::DeclarationIndex, ty: &ResolvedType) -> bool {
    if !is_step(ty)
        || !element(ty).is_some_and(|element| resolved_element_is_admitted_in(index, element))
    {
        return false;
    }
    let owner = DeclarationId::compiler(CompilerDeclaration::IterStep);
    let Some(cases) = index.variant_cases(&owner) else {
        return false;
    };
    let parameter = ResolvedType::TypeParameter {
        owner: owner.clone(),
        index: 0,
    };
    matches!(cases,[done,yielded] if done.id.as_str()==DONE_ID&&done.name=="Done"&&done.index==0&&done.fields.is_empty()
 &&yielded.id.as_str()==YIELD_ID&&yielded.name=="Yield"&&yielded.index==1
 &&matches!(yielded.fields.as_slice(),[item,rest] if item.id.as_str()==ITEM_ID&&item.name=="item"&&item.index==0&&item.ty==parameter
 &&rest.id.as_str()==REST_ID&&rest.name=="rest"&&rest.index==1&&rest.ty==resolved_iter(parameter)))
}

pub(crate) fn validate_declarations(
    program: &crate::hir::ResolvedProgram,
) -> Result<(), crate::diagnostic::Diagnostic> {
    use crate::hir::DeclarationKind;
    for id in [CompilerDeclaration::Iter, CompilerDeclaration::IterStep] {
        let id = DeclarationId::compiler(id);
        let Some(declaration) = program.declarations.declaration(&id) else {
            continue;
        };
        let valid = program
            .declarations
            .type_parameters(&id)
            .is_some_and(|p| p.len() == 1)
            && if id.as_str() == ITER_ID {
                declaration.kind == DeclarationKind::Record
                    && declaration.name == "Iter"
                    && program
                        .declarations
                        .record_fields(&id)
                        .is_some_and(|f| f.is_empty())
            } else {
                declaration.kind == DeclarationKind::Variant
                    && declaration.name == "IterStep"
                    && step_shape(
                        &program.declarations,
                        &resolved_iter_step(ResolvedType::I64),
                    )
            };
        if !valid {
            return Err(crate::diagnostic::Diagnostic::io(
                "SPX-H006",
                "iterator prelude declaration is not canonical",
            ));
        }
    }
    Ok(())
}

pub(crate) fn element(ty: &ResolvedType) -> Option<&ResolvedType> {
    match ty {
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if matches!(declaration.as_str(), ITER_ID | STEP_ID) => match arguments.as_slice() {
            [element] => Some(element),
            _ => None,
        },
        _ => None,
    }
}

pub(crate) fn is_iter(ty: &ResolvedType) -> bool {
    matches!(ty,ResolvedType::Nominal{declaration,..} if declaration.as_str()==ITER_ID)
        && element(ty).is_some()
}

pub(crate) fn is_step(ty: &ResolvedType) -> bool {
    matches!(ty,ResolvedType::Nominal{declaration,..} if declaration.as_str()==STEP_ID)
        && element(ty).is_some()
}

pub(crate) fn resolved_element_is_admitted(ty: &ResolvedType) -> bool {
    crate::vec_ops::resolved_element_is_admitted(ty) || *ty == ResolvedType::Bytes
}

pub(crate) fn resolved_element_is_admitted_in(
    declarations: &crate::hir::DeclarationIndex,
    ty: &ResolvedType,
) -> bool {
    resolved_element_is_admitted(ty)
        || crate::hir::owned_leaf_collection::runtime_element(declarations, ty)
}

fn resolved(id: CompilerDeclaration, element: ResolvedType) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::compiler(id),
        arguments: vec![element],
    }
}

pub(crate) fn resolved_iter(element: ResolvedType) -> ResolvedType {
    resolved(CompilerDeclaration::Iter, element)
}

pub(crate) fn resolved_iter_step(element: ResolvedType) -> ResolvedType {
    resolved(CompilerDeclaration::IterStep, element)
}
