//! Execution identity comes from the authenticated invocation, never the declaration alone.
use crate::diagnostic::Diagnostic;
use crate::hir::{FunctionExecutionId, ResolvedFunction, ResolvedProgram};
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
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<FunctionExecutionId, Diagnostic> {
    let mut found = None;
    for candidate in program
        .functions
        .iter()
        .map(|f| FunctionExecutionId::Monomorphic(f.id.clone()))
        .chain(
            program
                .function_instances
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
