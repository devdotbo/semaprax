//! Independent allocation branches in the complete ordinary path receiver.
//! The opaque current member can only come from the private source projection.
//! A decoded v20 plan is never a selector. Admission remains Ordinary.
use super::*;

pub(super) fn current<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
) -> Option<super::super::checked_allocation::Function<'a>> {
    let current = program.allocation_function(function)?;
    let full = program.full_program()?;
    current.held_by(full, function).then_some(current)
}

pub(super) fn source(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Option<StatusSourceId>, Diagnostic> {
    if current(program, function).is_none() {
        return Ok(None);
    }
    if ordinary_status::allocation::Selection::operation_with(
        expression,
        &work.string_owner_moves,
        &work.string_condition_reads,
    )
    .is_none()
    {
        return Ok(None);
    }
    Ok(Some(StatusSourceId {
        expression: work.clone_owned(&expression.id, "allocation failure source identity")?,
        lane: StatusLane::AllocationFailure,
    }))
}

/// Fork immediately at the allocation frontier. Failure keeps the already
/// recorded argument/semantic observations, but cannot initialize a result.
/// Caller epochs remain staged when this precedes a deferred CallCommit.
pub(super) fn split_call(
    results: &mut Vec<ExprSkeletonPath>,
    path: &mut ExprSkeletonPath,
    source: &StatusSourceId,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<(), Diagnostic> {
    let mut failure = work.clone_expr_path(path, "allocation failure path")?;
    let failure_source = work.clone_owned(source, "allocation failure source")?;
    work.push_observation(
        &mut failure,
        SkeletonObservation::Status {
            source: failure_source,
            success: false,
        },
        "allocation failure observation",
    )?;
    failure.failed = true;
    failure.owned_source = None;
    work.push_expr_path(results, failure, "allocation failed call path")?;
    let success_source = work.clone_owned(source, "allocation success source")?;
    work.push_observation(
        path,
        SkeletonObservation::Status {
            source: success_source,
            success: true,
        },
        "allocation success observation",
    )
}

pub(super) fn string_paths(
    expression: &ResolvedExpr,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    let source = StatusSourceId {
        expression: work.clone_owned(&expression.id, "String allocation status identity")?,
        lane: StatusLane::AllocationFailure,
    };
    let mut results = Vec::new();
    let mut path = empty_expr_path();
    split_call(&mut results, &mut path, &source, work)?;
    let destination = temporary_place(expression, work)?;
    let at = work.clone_owned(&expression.id, "String allocation initialization identity")?;
    let initialized = work.clone_owned(&destination, "String allocation destination")?;
    work.push_observation(
        &mut path,
        SkeletonObservation::Initialize {
            at,
            destination: initialized,
        },
        "String allocation initialization",
    )?;
    path.owned_source = Some(destination);
    work.push_expr_path(&mut results, path, "String allocation successful path")?;
    Ok(results)
}
