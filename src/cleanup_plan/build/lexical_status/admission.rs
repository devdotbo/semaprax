//! Borrowed native-owner admission comparison: no synthesized nominal types.
use super::*;
use crate::hir::ResolvedImportResultKind as R;
fn nominal<'a>(ty: &'a ResolvedType, id: Option<&DeclarationId>) -> Option<&'a [ResolvedType]> {
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return None;
    };
    (Some(declaration) == id).then_some(arguments.as_slice())
}
fn matches(kind: &R, ty: &ResolvedType, program: &ResolvedProgram) -> bool {
    let index = &program.declarations;
    match kind {
        R::OwnedString => *ty == ResolvedType::String,
        R::OwnedResource { resource } => {
            nominal(ty, Some(resource)).is_some_and(|args| args.is_empty())
        }
        R::OwnedOptionString => matches!(
            nominal(ty, index.type_id("Option")),
            Some([ResolvedType::String])
        ),
        R::OwnedResultStringI64 => matches!(
            nominal(ty, index.type_id("Result")),
            Some([ResolvedType::String, ResolvedType::I64])
        ),
        R::OwnedResultStringOptionI64 => match nominal(ty, index.type_id("Result")) {
            Some([ResolvedType::String, option]) => matches!(
                nominal(option, index.type_id("Option")),
                Some([ResolvedType::I64])
            ),
            _ => false,
        },
        R::OwnedResultResourceI64 { resource } => match nominal(ty, index.type_id("Result")) {
            Some([value, ResolvedType::I64]) => {
                nominal(value, Some(resource)).is_some_and(|args| args.is_empty())
            }
            _ => false,
        },
        R::Unit | R::I64 | R::Bool | R::BorrowedStr { .. } | R::ResultI64I64 => false,
    }
}
pub(super) fn required(
    program: &ResolvedProgram,
    expression: &ResolvedExpr,
    work: &Cell<usize>,
) -> Result<bool, Diagnostic> {
    let Some(call) = super::super::super::native_rust::parts(expression) else {
        return Ok(false);
    };
    if matches!(expression.kind, ResolvedExprKind::Call { .. }) {
        debit(
            work,
            if call.instance.is_some() {
                program.function_instances.len()
            } else {
                program.functions.len()
            },
        )?;
        if program
            .resolve_call_target(call.callee, call.instance)
            .is_none()
        {
            return Ok(false);
        }
    }
    for argument in call.args {
        debit(work, 1)?;
        if argument.ownership != OwnershipMode::Own {
            continue;
        }
        for interface in &program.interfaces {
            debit(work, 1)?;
            for import in &interface.imports {
                debit(work, 1)?;
                if import.native_rust && matches(&import.result.kind, &argument.ty, program) {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}
