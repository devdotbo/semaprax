//! Generic substitution preserves target/latch meaning while replacing execution scope.
use crate::diagnostic::Diagnostic;
use crate::hir::{ControlTarget, ExpressionId, FunctionExecutionId, LoopLatchKind, ValueId};
fn path<'a>(value: &'a str, from: &FunctionExecutionId, kind: &str) -> Result<&'a str, Diagnostic> {
    crate::hir::ids::checked_scoped_path(value, from, kind).ok_or_else(|| {
        Diagnostic::io(
            "SPX-H006",
            "lexical template identity has a foreign or noncanonical scope",
        )
    })
}
fn expression(
    value: &ExpressionId,
    from: &FunctionExecutionId,
    to: &FunctionExecutionId,
) -> Result<ExpressionId, Diagnostic> {
    Ok(ExpressionId::new(
        to,
        path(value.as_str(), from, "expression")?,
    ))
}
fn local(
    value: &ValueId,
    from: &FunctionExecutionId,
    to: &FunctionExecutionId,
) -> Result<ValueId, Diagnostic> {
    Ok(ValueId::local(
        to,
        path(value.as_str(), from, "value:local")?,
    ))
}
pub(crate) fn target(
    value: &ControlTarget,
    from: &FunctionExecutionId,
    to: &FunctionExecutionId,
) -> Result<ControlTarget, Diagnostic> {
    Ok(match value {
        ControlTarget::Function(owner) if owner == from => ControlTarget::Function(to.clone()),
        ControlTarget::Loop { owner, site } if owner == from => ControlTarget::Loop {
            owner: to.clone(),
            site: expression(site, from, to)?,
        },
        _ => {
            return Err(Diagnostic::io(
                "SPX-H006",
                "lexical template target names a foreign execution",
            ))
        }
    })
}
pub(crate) fn latch(
    value: &LoopLatchKind,
    from: &FunctionExecutionId,
    to: &FunctionExecutionId,
) -> Result<LoopLatchKind, Diagnostic> {
    Ok(match value {
        LoopLatchKind::Condition => LoopLatchKind::Condition,
        LoopLatchKind::SourceForStep { kind, step } => LoopLatchKind::SourceForStep {
            kind: kind.clone(),
            step: expression(step, from, to)?,
        },
        LoopLatchKind::OwnedIteratorStep {
            next,
            assignment,
            step,
            remainder,
        } => LoopLatchKind::OwnedIteratorStep {
            next: expression(next, from, to)?,
            assignment: expression(assignment, from, to)?,
            step: local(step, from, to)?,
            remainder: local(remainder, from, to)?,
        },
    })
}
