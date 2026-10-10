//! Explicit execution context for the selective lexical cleanup profile.
use super::*;
use crate::hir::FunctionExecutionId;

#[derive(Clone)]
pub(super) struct LexicalBuildState {
    pub owner: FunctionExecutionId,
    pub certificate: super::super::LexicalControlPlan,
}

/// Fresh monomorphic functions and late generic instances supply their actual
/// execution identity before registration. Legacy functions retain their exact
/// construction path and wire schema.
pub(crate) fn build_plan_for_execution(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    execution: &FunctionExecutionId,
) -> Result<CleanupPlan, Diagnostic> {
    crate::hir::lexical_exit::validate_owner(function, execution)?;
    let mut builder = PlanBuilder::new(program, function)?;
    if crate::hir::lexical_exit::requires(function)? {
        builder.schema = super::super::CLEANUP_PLAN_SCHEMA_V18;
        builder.lexical = Some(Box::new(LexicalBuildState {
            owner: execution.clone(),
            certificate: super::super::LexicalControlPlan {
                schema: "semaprax.lexical-control-flow.v1",
                loops: Vec::new(),
                exits: Vec::new(),
                resumes: Vec::new(),
                abrupt_only_regions: Vec::new(),
                function_success_exits: Vec::new(),
            },
        }));
    }
    builder.build()
}
