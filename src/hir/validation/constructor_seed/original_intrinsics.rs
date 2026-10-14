use super::*;

pub(super) fn reject_reserved_identities(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    super::super::vec_intrinsic::reject_reserved_identities(program)?;
    crate::iterator_ops::original_constructor::validate_declarations(program)?;
    crate::list_ops::original_constructor::validate_declarations(program)?;
    crate::map_ops::validate_declarations(program)?;
    for declaration in program.declarations.declarations() {
        if crate::map_ops::is_reserved_operation_id(declaration.id.as_str())
            || crate::map_ops::by_name(&declaration.name).is_some()
            || crate::list_ops::by_id(declaration.id.as_str()).is_some()
            || crate::list_ops::by_name(&declaration.name).is_some()
            || crate::iterator_ops::by_id(declaration.id.as_str()).is_some()
            || crate::iterator_ops::by_name(&declaration.name).is_some()
            || crate::box_ops::by_id(declaration.id.as_str()).is_some()
            || crate::box_ops::by_name(&declaration.name).is_some()
        {
            return Err(hir_error(format!(
                "resolved {:?} declaration `{}` aliases a compiler-owned box operation",
                declaration.kind, declaration.id
            )));
        }
    }
    for function in &program.functions {
        reject_function(function)?;
    }
    for template in &program.function_templates {
        if crate::map_ops::is_reserved_operation_id(template.id.as_str())
            || crate::map_ops::by_name(&template.name).is_some()
            || crate::list_ops::by_id(template.id.as_str()).is_some()
            || crate::list_ops::by_name(&template.name).is_some()
            || crate::iterator_ops::by_id(template.id.as_str()).is_some()
            || crate::iterator_ops::by_name(&template.name).is_some()
            || crate::box_ops::by_id(template.id.as_str()).is_some()
            || crate::box_ops::by_name(&template.name).is_some()
        {
            return Err(hir_error(format!(
                "resolved function template `{}` aliases a compiler-owned box operation",
                template.id
            )));
        }
    }
    for instance in &program.function_instances {
        reject_function(&instance.function)?;
    }
    Ok(())
}

fn reject_function(function: &ResolvedFunction) -> Result<(), Diagnostic> {
    if crate::map_ops::is_reserved_operation_id(function.id.as_str())
        || crate::map_ops::by_name(&function.name).is_some()
        || crate::list_ops::by_id(function.id.as_str()).is_some()
        || crate::list_ops::by_name(&function.name).is_some()
        || crate::iterator_ops::by_id(function.id.as_str()).is_some()
        || crate::iterator_ops::by_name(&function.name).is_some()
        || crate::box_ops::by_id(function.id.as_str()).is_some()
        || crate::box_ops::by_name(&function.name).is_some()
    {
        return Err(hir_error(format!(
            "resolved function `{}` aliases a compiler-owned box operation",
            function.id
        )));
    }
    Ok(())
}
