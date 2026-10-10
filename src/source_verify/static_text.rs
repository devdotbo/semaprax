//! Explicit immutable literal-root source admission. Ordinary String is unchanged.
use crate::ast::{ParamMode, Program, Span, Type};
use crate::diagnostic::Diagnostic;
pub(crate) const MESSAGE: &str = "static UTF-8 literal exceeds 65536 decoded bytes";
pub(super) fn checked(
    program: &Program,
    span: Span,
    text: &str,
) -> Result<super::binding::CheckedValue, Diagnostic> {
    if text.len() > crate::hir::static_text::MAX_POOL_BYTES {
        return Err(super::diagnostics::error(
            program, "SPX-T314", MESSAGE, span,
        ));
    }
    Ok(super::binding::CheckedValue {
        ty: Type::Str,
        mode: ParamMode::Borrow,
        native_unit: false,
    })
}
