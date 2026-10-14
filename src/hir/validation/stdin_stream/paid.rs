//! Paid streaming predicates. Inline slots are physical scratch, never admission.
use super::super::*;
use crate::hir::ordinary_scan::{self, Stack};
use crate::stdin_stream_ops as stream;
mod analysis;

pub(super) fn type_uses(ty: &ResolvedType) -> Result<bool, Diagnostic> {
    let mut pending = Stack::new()?;
    pending.push(ty)?;
    while let Some(ty) = pending.pop() {
        match ty {
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                if declaration.as_str() == stream::READER_ID {
                    return Ok(true);
                }
                for argument in arguments {
                    pending.push(argument)?;
                }
            }
            ResolvedType::Function { parameters, result } => {
                pending.push(result.as_ref())?;
                for parameter in parameters {
                    pending.push(parameter)?;
                }
            }
            _ => {}
        }
    }
    Ok(false)
}
pub(super) fn expression_uses(root: &ResolvedExpr) -> Result<bool, Diagnostic> {
    let mut pending = Stack::new()?;
    pending.push(root)?;
    while let Some(expression) = pending.pop() {
        if type_uses(&expression.ty)?
            || matches!(&expression.kind, ResolvedExprKind::HostCommandCall(call) if stream::is_host(call.operation))
            || matches!(&expression.kind, ResolvedExprKind::BorrowPlace { operation, .. } if operation.as_str() == stream::CHUNK_ID)
            || matches!(&expression.kind, ResolvedExprKind::Call { callee, .. } if stream::pure_by_id(callee.as_str()).is_some())
        {
            return Ok(true);
        }
        if let ResolvedExprKind::Closure { body, .. } = &expression.kind {
            pending.push(body.as_ref())?;
        }
        ordinary_scan::children(expression, &mut |child| pending.push(child))?;
    }
    Ok(false)
}
fn expressions_use<'a>(
    items: impl IntoIterator<Item = &'a ResolvedExpr>,
) -> Result<bool, Diagnostic> {
    for expression in items {
        if expression_uses(expression)? {
            return Ok(true);
        }
    }
    Ok(false)
}
fn parameters_use(params: &[ResolvedParam]) -> Result<bool, Diagnostic> {
    for param in params {
        if type_uses(&param.ty)? {
            return Ok(true);
        }
    }
    Ok(false)
}
fn import_parameters_use(params: &[ResolvedImportParameter]) -> Result<bool, Diagnostic> {
    for param in params {
        if type_uses(&param.ty)? {
            return Ok(true);
        }
    }
    Ok(false)
}
fn fields_use(fields: &[ResolvedFieldDeclaration]) -> Result<bool, Diagnostic> {
    for field in fields {
        if type_uses(&field.ty)? {
            return Ok(true);
        }
    }
    Ok(false)
}
fn function_uses(function: &ResolvedFunction) -> Result<bool, Diagnostic> {
    if type_uses(&function.return_type)? || parameters_use(&function.params)? {
        return Ok(true);
    }
    expressions_use(
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures),
    )
}
fn program_uses(program: &ResolvedProgram) -> Result<bool, Diagnostic> {
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
    {
        if function_uses(function)? {
            return Ok(true);
        }
    }
    Ok(false)
}
fn captures_use(captures: &[ResolvedClosureCapture]) -> Result<bool, Diagnostic> {
    for capture in captures {
        if type_uses(&capture.value.ty)? {
            return Ok(true);
        }
    }
    Ok(false)
}
fn types_use(types: &[ResolvedType]) -> Result<bool, Diagnostic> {
    for ty in types {
        if type_uses(ty)? {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(in crate::hir::validation) fn reject_sealed_escape_paid(
    program: &ResolvedProgram,
) -> Result<(), Diagnostic> {
    use crate::stdin_stream_ops::is_reader;
    for declaration in program.declarations.declarations() {
        if crate::stdin_stream_ops::pure_by_id(declaration.id.as_str()).is_some()
            || crate::stdin_stream_ops::pure_by_name(&declaration.name).is_some()
        {
            return Err(hir_error_static(
                "authored declaration aliases streaming stdin inspection",
            ));
        }
    }
    for declaration in &program.types {
        if declaration.id.as_str() == crate::stdin_stream_ops::READER_ID {
            if declaration.name != "StdinReader"
                || !declaration.type_parameters.is_empty()
                || !matches!(&declaration.kind, ResolvedTypeDeclarationKind::Record { fields } if fields.is_empty())
                || !program
                    .declarations
                    .declaration(&declaration.id)
                    .is_some_and(|d| d.identity_origin == IdentityOrigin::CompilerOwned)
            {
                return Err(hir_error_static(
                    "streaming reader declaration is not sealed compiler-owned metadata",
                ));
            }
            continue;
        }
        let uses = match &declaration.kind {
            ResolvedTypeDeclarationKind::Record { fields }
            | ResolvedTypeDeclarationKind::Class { fields, .. } => fields_use(fields)?,
            ResolvedTypeDeclarationKind::Variant { cases } => {
                let mut found = false;
                for case in cases {
                    if fields_use(&case.fields)? {
                        found = true;
                        break;
                    }
                }
                found
            }
            ResolvedTypeDeclarationKind::Resource { .. } => false,
        };
        if uses {
            return Err(hir_error_static(
                "streaming reader cannot be stored in an aggregate",
            ));
        }
    }
    for import in program
        .interfaces
        .iter()
        .flat_map(|interface| &interface.imports)
    {
        if matches!(&import.result.kind, ResolvedImportResultKind::OwnedResource { resource } | ResolvedImportResultKind::OwnedResultResourceI64 { resource } | ResolvedImportResultKind::BorrowedStr { resource } if resource.as_str() == crate::stdin_stream_ops::READER_ID)
            || import_parameters_use(&import.parameters)?
        {
            return Err(hir_error_static(
                "streaming reader cannot cross an authored import ABI",
            ));
        }
    }
    for template in &program.function_templates {
        if type_uses(&template.return_type)? || parameters_use(&template.params)? {
            return Err(hir_error_static(
                "streaming reader cannot appear in a generic function template",
            ));
        }
    }
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
    {
        if type_uses(&function.return_type)? && !is_reader(&function.return_type) {
            return Err(hir_error_static(
                "streaming reader cannot appear inside a return carrier",
            ));
        }
        for param in &function.params {
            if type_uses(&param.ty)?
                && (!is_reader(&param.ty)
                    || !matches!(param.ownership, OwnershipMode::Own | OwnershipMode::Borrow))
            {
                return Err(hir_error_static(
                    "streaming reader parameters require exact own or borrow mode",
                ));
            }
        }
        if function_uses(function)? && function.yields.is_some() {
            return Err(hir_error_static(
                "streaming reader functions must be synchronous",
            ));
        }
        if expressions_use(function.requires.iter().chain(&function.ensures))? {
            return Err(hir_error_static(
                "streaming stdin operations are not admitted in contracts",
            ));
        }
        let mut pending = Stack::new()?;
        pending.push(&function.body)?;
        for expression in &function.requires {
            pending.push(expression)?;
        }
        for expression in &function.ensures {
            pending.push(expression)?;
        }
        while let Some(expression) = pending.pop() {
            if type_uses(&expression.ty)? && !is_reader(&expression.ty) {
                return Err(hir_error_static(
                    "streaming reader cannot appear in a generic or callable carrier",
                ));
            }
            match &expression.kind {
                ResolvedExprKind::ConstructRecord { .. } | ResolvedExprKind::ConstructVariant { .. }
                | ResolvedExprKind::UpdateRecord { .. } | ResolvedExprKind::If { .. }
                    if is_reader(&expression.ty) => return Err(hir_error_static("streaming reader has no construction, update, or conditional-result operation")),
                ResolvedExprKind::Match { scrutinee, .. } if is_reader(&scrutinee.ty) => return Err(hir_error_static("streaming reader cannot be pattern matched")),
                ResolvedExprKind::Project { base, .. } if is_reader(&base.ty) => return Err(hir_error_static("streaming reader cannot be projected")),
                ResolvedExprKind::Call { type_arguments, .. } if types_use(type_arguments)? => return Err(hir_error_static("streaming reader cannot instantiate a generic call")),
                ResolvedExprKind::Closure { captures, body, .. } => {
                    if captures_use(captures)? { return Err(hir_error_static("streaming reader cannot be captured")); }
                    pending.push(body.as_ref())?;
                }
                ResolvedExprKind::Yield { request } if type_uses(&request.ty)? => return Err(hir_error_static("streaming reader cannot cross suspension")),
                _ => {}
            }
            ordinary_scan::children(expression, &mut |child| pending.push(child))?;
        }
    }
    analysis::derive(program)?;
    Ok(())
}

#[cfg(test)]
mod tests;
