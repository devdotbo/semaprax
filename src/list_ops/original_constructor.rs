use super::*;

pub(crate) fn is_list(ty: &ResolvedType) -> bool {
    *ty == resolved_list()
}

pub(crate) fn is_step(ty: &ResolvedType) -> bool {
    *ty == resolved_step()
}

pub(crate) fn step_shape(index: &crate::hir::DeclarationIndex, ty: &ResolvedType) -> bool {
    if !is_step(ty) {
        return false;
    }
    let owner = DeclarationId::compiler(CompilerDeclaration::ListStep);
    let Some(cases) = index.variant_cases(&owner) else {
        return false;
    };
    let parameter = ResolvedType::TypeParameter {
        owner: owner.clone(),
        index: 0,
    };
    matches!(cases, [nil, cons]
        if nil.id.as_str() == NIL_CASE_ID && nil.name == "Nil" && nil.index == 0 && nil.fields.is_empty()
        && cons.id.as_str() == CONS_CASE_ID && cons.name == "Cons" && cons.index == 1
        && matches!(cons.fields.as_slice(), [head, tail]
            if head.id.as_str() == HEAD_ID && head.name == "head" && head.index == 0 && head.ty == parameter
            && tail.id.as_str() == TAIL_ID && tail.name == "tail" && tail.index == 1
            && tail.ty == ResolvedType::Nominal { declaration: DeclarationId::compiler(CompilerDeclaration::List), arguments: vec![parameter] }))
}

pub(crate) fn validate_declarations(
    program: &crate::hir::ResolvedProgram,
) -> Result<(), crate::diagnostic::Diagnostic> {
    use crate::hir::DeclarationKind;
    for (id, key) in [
        (LIST_ID, CompilerDeclaration::List),
        (STEP_ID, CompilerDeclaration::ListStep),
    ] {
        let key = DeclarationId::compiler(key);
        let Some(declaration) = program.declarations.declaration(&key) else {
            continue;
        };
        let valid = declaration.identity_origin == crate::hir::IdentityOrigin::CompilerOwned
            && program
                .declarations
                .type_parameters(&key)
                .is_some_and(|parameters| parameters.len() == 1)
            && if id == LIST_ID {
                declaration.kind == DeclarationKind::Record
                    && declaration.name == "List"
                    && program
                        .declarations
                        .record_fields(&key)
                        .is_some_and(|fields| fields.is_empty())
            } else {
                declaration.kind == DeclarationKind::Variant
                    && declaration.name == "ListStep"
                    && step_shape(&program.declarations, &resolved_step())
            };
        if !valid {
            return Err(crate::diagnostic::Diagnostic::io(
                "SPX-H006",
                "immutable list prelude declaration is not canonical",
            ));
        }
    }
    Ok(())
}

pub(crate) fn resolved_list() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::compiler(CompilerDeclaration::List),
        arguments: vec![ResolvedType::I64],
    }
}

pub(crate) fn resolved_step() -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::compiler(CompilerDeclaration::ListStep),
        arguments: vec![ResolvedType::I64],
    }
}
