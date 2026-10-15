//! A no-result statement boundary, never inferred from a binding's spelling.
use super::{hir_error, OwnershipMode, ResolvedExpr, ResolvedType};
use crate::diagnostic::Diagnostic;

/// Type/storage check only. Exact expression identity, lexical targets,
/// ownership availability and cleanup are independent receiving obligations.
pub(super) fn require(value: &ResolvedExpr) -> Result<(), Diagnostic> {
    if !matches!(value.kind, super::ResolvedExprKind::Match { .. })
        || value.ownership != OwnershipMode::Value
        || !(super::is_scalar_resolved_type(&value.ty) || value.ty == ResolvedType::Never)
    {
        return Err(hir_error(
            "discarded expression statement requires a Copy scalar or lexical completion",
        ));
    }
    Ok(())
}

fn expression_uses(expression: &super::ResolvedExpr, depth: usize) -> bool {
    if depth > crate::cache_codec::MAX_DEPTH {
        // The mandatory validator rejects this subject; never report absence.
        return true;
    }
    if matches!(&expression.kind, super::ResolvedExprKind::Block { statements, .. }
        | super::ResolvedExprKind::VecConstructor { statements, .. }
        if statements.iter().any(|s|matches!(s,super::ResolvedStatement::CopyDiscard { .. })))
    {
        return true;
    }
    let mut children = super::lexical_exit::ChildCursor::new(expression);
    children.any(|child| expression_uses(child, depth + 1))
}
pub(crate) fn function_uses(function: &super::ResolvedFunction) -> bool {
    function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
        .any(|expression| expression_uses(expression, 0))
}
pub(crate) fn program_uses(program: &super::ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(function_uses)
}

/// Only an authenticated expression-statement marker with a lexical descendant
/// lowers to the selective statement representation.
pub(crate) fn source(statement: &crate::ast::Statement) -> bool {
    crate::ast::expression_statement::value(statement).is_some_and(|value| {
        matches!(value.kind, crate::ast::ExprKind::Match { .. }) && value.contains_lexical_exit()
    })
}
