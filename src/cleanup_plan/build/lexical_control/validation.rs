//! The existing lexical state initializer for an independently selected owner.
use super::*;
pub(in crate::cleanup_plan::build) fn install(
    builder: &mut PlanBuilder<'_>,
    execution: &FunctionExecutionId,
) {
    if builder.schema != super::super::super::CLEANUP_PLAN_SCHEMA_V19 {
        builder.schema = super::super::super::CLEANUP_PLAN_SCHEMA_V18;
    }
    builder.lexical = Some(Box::new(LexicalBuildState {
        owner: execution.clone(),
        returns: Vec::new(),
        active_loops: Vec::new(),
        wrapper_breaks: Vec::new(),
        building_resume: false,
        replicated_statuses: BTreeSet::new(),
        status_normal_visits: Vec::new(),
        status_work: 0,
        ensure_regions: Vec::new(),
        current_ensure: None,
        certificate: super::super::super::LexicalControlPlan {
            schema: "semaprax.lexical-control-flow.v1",
            loops: Vec::new(),
            exits: Vec::new(),
            resumes: Vec::new(),
            abrupt_only_regions: Vec::new(),
            function_success_exits: Vec::new(),
            execution_boundaries: Vec::new(),
        },
    }));
}
