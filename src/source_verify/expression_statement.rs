//! Syntax provenance never grants ownership, borrowed aliases or unit discard.
use super::diagnostics::{error, is_scalar_source_type};
use super::CheckedValue;
use crate::ast::program_source::ProgramSource;
use crate::ast::{LetSyntax, ParamMode, Statement};
use crate::diagnostic::Diagnostic;
pub(super) fn check(
    program: &dyn ProgramSource,
    statement: &Statement,
    actual: &CheckedValue,
) -> Option<Diagnostic> {
    let Statement::Let {
        syntax: LetSyntax::ExpressionStatement,
        span,
        ..
    } = statement
    else {
        return None;
    };
    if crate::ast::expression_statement::value(statement).is_none()
        || actual.mode != ParamMode::Value
        || actual.native_unit
        || !(is_scalar_source_type(&actual.ty)
            || (actual.ty == crate::ast::Type::Never
                && crate::ast::expression_statement::value(statement).is_some_and(|value| {
                    matches!(value.kind, crate::ast::ExprKind::Match { .. })
                        && crate::ast::has_no_normal_completion(value)
                })))
    {
        Some(error(program, "SPX-T315", "discarded expression statement requires a Copy scalar result", *span)
            .with_help("bind the result explicitly; only i64, i32, u8, usize, char, f32, f64 and bool may be discarded"))
    } else {
        None
    }
}
