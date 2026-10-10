//! A selected primitive invocation borrows its exact current function member.
//! Avoid cloning every rejected owner and allocating discarded diagnostics.
use super::*;
pub(super) fn owner(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<crate::hir::FunctionExecutionId, Diagnostic> {
    if budget.physical.is_none() {
        return crate::hir::lexical_exit::owner(program, function);
    }
    if !program.function_templates().is_empty() || !program.function_instances().is_empty() {
        return Err(replay_error(
            function,
            "selected lexical owner requires a module-local monomorphic invocation",
        ));
    }
    let mut found = false;
    for current in program.functions() {
        budget.charge(function, 1, "selected current function membership")?;
        if std::ptr::eq(current, function) {
            if found {
                return Err(replay_error(
                    function,
                    "selected lexical function member is repeated",
                ));
            }
            found = true;
        }
    }
    if !found {
        return Err(replay_error(
            function,
            "selected lexical function is not held by its current context",
        ));
    }
    budget.reserve_clone(function, &function.id, "selected execution owner")?;
    let owner = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
    crate::hir::lexical_exit::validate_owner(function, &owner)?;
    Ok(owner)
}
