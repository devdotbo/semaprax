//! Independent source root admission; typing and argument evaluation stay ordinary.
use super::binding::{Availability, Binding};
use crate::ast::{Expr, ExprKind, ParamMode, Type};
use std::collections::HashMap;
// Shape-only loop admission; ordinary typing still authenticates the live owner.
pub(super) fn while_shape(types: &[Type], args: &[Expr]) -> bool {
    types.is_empty() && matches!(args, [source, _] if matches!(source.kind, ExprKind::Var(_)))
}
pub(super) fn root<'a>(
    types: &[Type],
    args: &[Expr],
    variables: &'a HashMap<String, Binding>,
) -> Option<&'a str> {
    if !types.is_empty() || args.len() != 2 {
        return None;
    }
    let ExprKind::Var(name) = &args[0].kind else {
        return None;
    };
    let (held, binding) = variables.get_key_value(name)?;
    (binding.ty == crate::vec_ops::ast_vec(Type::String)
        && matches!(binding.mode, ParamMode::Own | ParamMode::Borrow)
        && binding.availability == Availability::Available
        && binding.moved_places.is_empty()
        && binding.definitely_partial.is_empty())
    .then_some(held.as_str())
}

/// Failure-only test telemetry; recognized operation names are static compiler data.
/// No operand text, values, or binding inventory is emitted.
#[cfg(test)]
pub(super) fn trace_while_refusal(
    phase: &str,
    operation: crate::vec_ops::VecOp,
    types: &[Type],
    args: &[Expr],
) {
    eprintln!(
        "T283-operand-v1 phase={phase} name={} op={} admitted={} types={} args={} named_root={}",
        operation.name(),
        operation.id(),
        operation.admitted_in_while(),
        types.len(),
        args.len(),
        matches!(args.first().map(|arg| &arg.kind), Some(ExprKind::Var(_))),
    );
}
