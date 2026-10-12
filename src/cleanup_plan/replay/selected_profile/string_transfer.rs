//! Independently reconstruct the closed named String transfer shape. Availability,
//! exact current root and canonical argument epochs remain separate receivers.
use super::*;
pub(in crate::cleanup_plan::replay) fn operand<'a>(
    function: &ResolvedFunction,
    expression: &'a ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<Option<&'a ResolvedExpr>, Diagnostic> {
    let ResolvedExprKind::Call { callee, .. } = &expression.kind else {
        return Ok(None);
    };
    budget.charge(
        function,
        callee.as_str().len().min(crate::string_ops::take::ID.len()) + 1,
        "selected String transfer operation identity",
    )?;
    if callee.as_str() != crate::string_ops::take::ID {
        return Ok(None);
    }
    budget.charge(
        function,
        crate::string_ops::take::ID.len() + 2,
        "selected String transfer exact operand shape and repeated identity",
    )?;
    crate::string_ops::take::operand(expression)
        .map(Some)
        .ok_or_else(|| {
            replay_error(
                function,
                "selected String transfer requires one named owned String",
            )
        })
}
