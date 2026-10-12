//! Machine-oracle receiver for an exact-source checked allocation projection.
//!
//! The ordinary entry correctly refuses v20 plans because an ordinary context
//! never selects allocation replay. This receiver accepts only the private
//! validated owner and repeats its complete projected replay before running.
//! Scenario allocation outcomes are deterministic semantic injections at the
//! exact canonical sources, not physical allocator evidence. The trace lends no
//! allocator, release lease, engine route or publication authority.
#![cfg_attr(
    not(test),
    allow(dead_code, reason = "checked allocation receivers are not activated")
)]
use super::*;

pub(in crate::cleanup_plan) fn execute_for_conformance(
    validated: &crate::cleanup_plan::checked_allocation::Validated<'_>,
    function: &DeclarationId,
    scenario: CleanupScenario,
) -> Result<ConformanceTrace, CleanupExecutionError> {
    let invalid = |diagnostic: crate::diagnostic::Diagnostic| {
        CleanupExecutionError::InvalidProgram(diagnostic.to_string())
    };
    let program = validated.program();
    hir::validate_attached_identity_references(program).map_err(invalid)?;
    crate::cleanup::validate_program(program).map_err(invalid)?;
    validated.replay().map_err(invalid)?;
    let function = program
        .functions
        .iter()
        .find(|candidate| candidate.id == *function)
        .ok_or_else(|| CleanupExecutionError::FunctionNotFound(function.clone()))?;
    if function.cleanup_plan.schema != super::super::CLEANUP_PLAN_SCHEMA_V20 {
        return Err(invariant(
            "checked allocation oracle received an ordinary plan",
        ));
    }
    validate_public_result_type(program, function)?;
    Executor::new(program, function, scenario)?.run()
}

pub(super) fn failure_status(
    executor: &mut Executor<'_>,
    source: &StatusSourceId,
    cases: [crate::cleanup_plan::AllocationCase; 2],
) -> Result<NormalizedStatus, CleanupExecutionError> {
    let status = executor.failure_outcome(source)?;
    if source.lane != StatusLane::AllocationFailure
        || cases
            != [
                crate::cleanup_plan::AllocationCase::PayloadRefused,
                crate::cleanup_plan::AllocationCase::SizeOverflow,
            ]
        || status.domain_id() != crate::cleanup_plan::ALLOCATION_DOMAIN_V1
        || !cases.iter().any(|case| status.code() == case.code())
        || status.class() != crate::conformance::StatusClass::Adapter
        || status.retryability() != crate::conformance::Retryability::Known(false)
    {
        return Err(invariant(
            "allocation failure supplied a status outside its exact refusal domain",
        ));
    }
    Ok(status)
}

/// Recognize a builtin only in the already independently replayed private v20
/// projection. A supplied outcome is never a declaration or allocator grant.
pub(super) fn is_intrinsic_call(executor: &Executor<'_>, expression: &hir::ResolvedExpr) -> bool {
    if executor.function.cleanup_plan.schema != crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V20 {
        return false;
    }
    let hir::ResolvedExprKind::Call { instance: None, .. } = &expression.kind else {
        return false;
    };
    let Some(expected) = crate::cleanup_plan::build::checked_allocation::operation(expression) else {
        return false;
    };
    executor.function.cleanup_plan.status_sources.iter().any(|source| {
        source.id.expression == expression.id
            && source.id.lane == StatusLane::AllocationFailure
            && matches!(&source.producer,
                StatusProducer::CheckedAllocation { operation, normalized_cases }
                    if *operation == expected && *normalized_cases == [
                        crate::cleanup_plan::AllocationCase::PayloadRefused,
                        crate::cleanup_plan::AllocationCase::SizeOverflow,
                    ])
    })
}
