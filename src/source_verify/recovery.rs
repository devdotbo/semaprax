//! Recovery after a `let` whose initializer was already refused.
//!
//! Such a `let` has no checked type, so it declares no binding and every later
//! read of its name would report `SPX-T202` for a name the source does
//! declare. This reads the authored function instead of adding verifier state:
//! a read is a follow-on only when an earlier `let` of an enclosing block
//! declares the name and an error already lies inside that initializer.
//! The refused program stays refused; only its repeated symptom is dropped.

use crate::ast::{Expr, ExprKind, Function, Span, Statement};
use crate::diagnostic::Diagnostic;

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// `unknown` is the `SPX-T202` about to be reported for `name`. Returns `true`
/// when an in-scope `let` declares `name` and its initializer already carries
/// an error in the same source file.
pub(super) fn follows_refused_let(
    current: &Function,
    name: &str,
    unknown: &Diagnostic,
    diagnostics: &[Diagnostic],
) -> bool {
    let Some(used) = unknown.span else {
        return false;
    };
    let refused = |initializer: &Expr| {
        diagnostics.iter().any(|reported| {
            reported.severity.is_error()
                && reported.path == unknown.path
                && reported
                    .span
                    .is_some_and(|span| contains(initializer.span, span))
        })
    };
    let mut found = false;
    for root in current
        .requires
        .iter()
        .chain(std::iter::once(&current.body))
        .chain(&current.ensures)
    {
        root.visit_all_nodes(&mut |expression| {
            let ExprKind::Block { statements, .. } = &expression.kind else {
                return;
            };
            if found || !contains(expression.span, used) {
                return;
            }
            // Shadowing is refused (`SPX-T209`), so an earlier `let` of an
            // enclosing block is the one declaration this read can mean.
            found = statements.iter().any(|statement| {
                matches!(
                    statement,
                    Statement::Let { name: declared, value, span, .. }
                        if declared == name && span.end <= used.start && refused(value)
                )
            });
        });
    }
    found
}
