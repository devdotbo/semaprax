//! Provisional semantic refusal: syntax alone cannot grant a static text root.
use crate::ast::{Program, Span};
use crate::diagnostic::Diagnostic;
pub(crate) const MESSAGE: &str = "borrowed UTF-8 literal syntax has no authenticated static-root implementation yet";
pub(super) fn unsupported(program: &Program, span: Span) -> Diagnostic {
    super::diagnostics::error(program, "SPX-T314", MESSAGE, span)
        .with_help("Keep this future spelling out of executable source until source/HIR, lifetime and backend qualification are complete; ordinary String literals retain their existing ownership rules")
}
