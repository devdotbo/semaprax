//! Execution identity comes from the authenticated invocation, never the declaration alone.
use crate::diagnostic::Diagnostic;
use crate::hir::{FunctionExecutionId, ResolvedFunction};
pub(crate) fn validate_owner(
    function: &ResolvedFunction,
    owner: &FunctionExecutionId,
) -> Result<(), Diagnostic> {
    if function.result_id.matches_result(owner) && function.body.id.matches(owner, "body") {
        Ok(())
    } else {
        Err(Diagnostic::io(
            "SPX-H006",
            "lexical function owner differs from canonical result/body identities",
        ))
    }
}
pub(crate) fn owner(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) -> Result<FunctionExecutionId, Diagnostic> {
    let mut found = None;
    for candidate in program
        .functions()
        .iter()
        .map(|f| FunctionExecutionId::Monomorphic(f.id.clone()))
        .chain(
            program
                .function_instances()
                .iter()
                .map(|i| FunctionExecutionId::Generic(i.id.clone())),
        )
    {
        if validate_owner(function, &candidate).is_ok() && found.replace(candidate).is_some() {
            return Err(Diagnostic::io(
                "SPX-H006",
                "lexical function execution owner is ambiguous",
            ));
        }
    }
    found.ok_or_else(|| {
        Diagnostic::io(
            "SPX-H006",
            "unregistered lexical function requires an explicit authenticated execution context",
        )
    })
}

#[cfg(test)]
pub(crate) fn validate_monomorphic_owner_with_work(
    function: &ResolvedFunction,
    owner: &FunctionExecutionId,
    charge: &mut impl FnMut(usize) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    if crate::hir::ids::monomorphic_owner_matches_with_work(function, owner, charge)? {
        Ok(())
    } else {
        Err(Diagnostic::io(
            "SPX-H006",
            "lexical function owner differs from canonical result/body identities",
        ))
    }
}
