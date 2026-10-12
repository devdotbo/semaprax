//! Recovery after a declaration or pattern the verifier already refused.
//!
//! A `let` whose initializer has no checked type declares no binding, and a
//! refused match pattern binds none of its names, so every later read would
//! report `SPX-T202` for a name the source does declare. A parameter or result
//! whose declared type is unknown would likewise fail every use against a type
//! nobody wrote. These helpers read the authored function and the diagnostics
//! already reported instead of adding verifier state: a later diagnostic is a
//! follow-on only when an error already lies inside the declaring initializer,
//! pattern, parameter or result type. The refused program stays refused; only
//! its repeated symptom is dropped.

use super::declared_type::check_declared_type;
use super::type_table::TypeTable;
use crate::ast::program_source::ProgramSource;
use crate::ast::{
    Expr, ExprKind, Function, MatchArm, MatchPattern, RecordMatchFieldPattern, Span, Statement,
    Type,
};
use crate::diagnostic::Diagnostic;
use std::collections::HashSet;

fn contains(outer: Span, inner: Span) -> bool {
    outer.start <= inner.start && inner.end <= outer.end
}

/// An error already reported inside `span` of the source file at `path`.
fn refused(span: Span, path: Option<&str>, diagnostics: &[Diagnostic]) -> bool {
    diagnostics.iter().any(|reported| {
        reported.severity.is_error()
            && reported.path.as_deref() == path
            && reported.span.is_some_and(|inner| contains(span, inner))
    })
}

/// Whether `pattern` binds `name`, at any depth.
fn binds(pattern: &MatchPattern, name: &str) -> bool {
    let mut patterns = vec![pattern];
    let mut records = Vec::new();
    while let Some(pattern) = patterns.pop() {
        match pattern {
            MatchPattern::Binding { name: bound, .. } if bound == name => return true,
            MatchPattern::Variant { fields, .. } => {
                if fields.iter().any(|field| field.binding == name) {
                    return true;
                }
            }
            MatchPattern::Record { fields, .. } => records.push(fields),
            MatchPattern::Or { alternatives, .. } => patterns.extend(alternatives),
            MatchPattern::Binding { .. }
            | MatchPattern::Wildcard { .. }
            | MatchPattern::Literal { .. } => {}
        }
    }
    while let Some(fields) = records.pop() {
        for field in fields {
            match &field.pattern {
                RecordMatchFieldPattern::Binding { name: bound, .. } if bound == name => {
                    return true
                }
                RecordMatchFieldPattern::Record { fields, .. } => records.push(fields),
                RecordMatchFieldPattern::Binding { .. }
                | RecordMatchFieldPattern::Wildcard { .. } => {}
            }
        }
    }
    false
}

/// `unknown` is the `SPX-T202` about to be reported for `name`. Returns `true`
/// when an in-scope `let` or enclosing match pattern declares `name` and an
/// error is already reported inside that initializer or pattern.
pub(super) fn follows_refused_binding(
    current: &Function,
    name: &str,
    unknown: &Diagnostic,
    diagnostics: &[Diagnostic],
) -> bool {
    let Some(used) = unknown.span else {
        return false;
    };
    let path = unknown.path.as_deref();
    let mut found = false;
    for root in current
        .requires
        .iter()
        .chain(std::iter::once(&current.body))
        .chain(&current.ensures)
    {
        root.visit_all_nodes(&mut |expression: &Expr| {
            if found || !contains(expression.span, used) {
                return;
            }
            found = match &expression.kind {
                // Shadowing is refused (`SPX-T209`), so an earlier `let` of an
                // enclosing block is the one declaration this read can mean.
                ExprKind::Block { statements, .. } => statements.iter().any(|statement| {
                    matches!(
                        statement,
                        Statement::Let { name: declared, value, span, .. }
                            if declared == name
                                && span.end <= used.start
                                && refused(value.span, path, diagnostics)
                    )
                }),
                ExprKind::Match { arms, .. } => arms.iter().any(|arm| {
                    contains(arm.span, used)
                        && binds(&arm.pattern, name)
                        && refused(arm.pattern.span(), path, diagnostics)
                }),
                _ => false,
            };
        });
    }
    found
}

/// A read of a parameter whose declared type names no declared type
/// (`SPX-T001`). The parameter keeps its binding; only the read loses that
/// unknown type. A type that exists but is refused (`SPX-T223`) and an
/// ownership-mode error keep the declared type in force.
pub(super) fn reads_refused_parameter(
    program: &dyn ProgramSource,
    current: &Function,
    name: &str,
    types: &TypeTable<'_>,
) -> bool {
    current.params.iter().any(|param| {
        param.name == name && declared_type_unknown(program, current, &param.ty, types)
    })
}

/// Coverage cannot be judged for a match one of whose patterns is refused.
/// `missing` is the `SPX-M101` about to be reported for that match.
pub(super) fn has_refused_pattern(
    arms: &[MatchArm],
    missing: &Diagnostic,
    diagnostics: &[Diagnostic],
) -> bool {
    let path = missing.path.as_deref();
    arms.iter()
        .any(|arm| refused(arm.pattern.span(), path, diagnostics))
}

/// Whether `declared` names a type that does not exist (`SPX-T001`). It
/// repeats the declaration pass's own check into a scratch list, so a `true`
/// here is an error that pass reports for the same signature.
pub(super) fn declared_type_unknown(
    program: &dyn ProgramSource,
    function: &Function,
    declared: &Type,
    types: &TypeTable<'_>,
) -> bool {
    let parameters = function
        .type_parameters
        .iter()
        .map(|parameter| parameter.name.as_str())
        .collect::<HashSet<_>>();
    let mut scratch = Vec::new();
    check_declared_type(
        program,
        declared,
        function.span,
        types,
        &parameters,
        &mut scratch,
    );
    scratch
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T001")
}
