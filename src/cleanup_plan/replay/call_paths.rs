//! Ordered call commit/status/result skeleton receiving.
use super::*;

pub(super) fn finish_call_states(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    states: Vec<CallSkeletonState>,
    work: &mut SkeletonWork<'_, '_>,
) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
    #[cfg(test)]
    let borrowed_total = match program.borrowed_helper_lease(function) {
        Some(lease) => Some(lease.call_is_total(program, function, expression, work.budget)?),
        None => None,
    };
    #[cfg(not(test))]
    let borrowed_total: Option<bool> = None;
    let infallible_compiler_operation = match borrowed_total {
        Some(total) => total,
        None => crate::cleanup_plan::deferred_commit::expression_is_infallible_compiler_operation(expression),
    };
    // `vec_push` and the one fallible byte operation both check their bound before the owner transfer commits, so their status observation precedes the
    // call-commit and a failed call leaves the owner in its call-argument slot for the ordinary region cleanup.
    // The complete held profile excludes deferred owner-transfer opcodes.
    let defer_commit = borrowed_total.is_none() && resolved_call::defers_owner_commit(expression);
    let infallible_compiler_operation = infallible_compiler_operation
        || matches!(
            &expression.kind,
            ResolvedExprKind::HostCommandCall(call)
                if crate::command_io_ops::failure(call.operation)
                    == crate::command_io_ops::CommandIoFailure::Infallible
        );
    let source_expression =
        work.clone_owned(&expression.id, "call status source expression clone")?;
    let source = StatusSourceId {
        expression: source_expression,
        lane: StatusLane::OperationFailure,
    };
    let mut results = Vec::new();
    for (mut path, commits) in states {
        if path.failed || path.residual {
            work.push_expr_path(&mut results, path, "short-circuited call path")?;
            continue;
        }
        if borrowed_total.is_none() && crate::cleanup_plan::owner_admission::required(program, expression) {
            let admission = StatusSourceId {
                expression: work.clone_owned(&expression.id, "owner-admission identity clone")?,
                lane: StatusLane::OwnerAdmission,
            };
            let mut refused = work.clone_expr_path(&path, "owner-admission failure path")?;
            work.push_observation(
                &mut refused,
                SkeletonObservation::Status {
                    source: admission.clone(),
                    success: false,
                },
                "owner-admission failure",
            )?;
            refused.failed = true;
            refused.owned_source = None;
            work.push_expr_path(&mut results, refused, "owner-admission failed path")?;
            work.push_observation(
                &mut path,
                SkeletonObservation::Status {
                    source: admission,
                    success: true,
                },
                "owner-admission success",
            )?;
        }
        if !defer_commit {
            let call = work.clone_owned(&expression.id, "call-commit identity clone")?;
            let arguments = work.clone_payload(&commits, "call-commit argument payload")?;
            work.push_observation(
                &mut path,
                SkeletonObservation::CallCommit { call, arguments },
                "call-commit observation",
            )?;
        }
        if infallible_compiler_operation {
            if let Some(source) = allocation_paths::source(program, function, expression, work)? {
                allocation_paths::split_call(&mut results, &mut path, &source, work)?;
            }
            if expression.ownership == OwnershipMode::Own
                && type_needs_drop(program, function, &expression.ty)?
            {
                let destination = temporary_place(expression, work)?;
                let at = work.clone_owned(&expression.id, "byte result expression clone")?;
                let initialized =
                    work.clone_owned(&destination, "byte result destination clone")?;
                work.push_observation(
                    &mut path,
                    SkeletonObservation::Initialize {
                        at,
                        destination: initialized,
                    },
                    "owned infallible byte result initialization",
                )?;
                path.owned_source = Some(destination);
            } else {
                path.owned_source = None;
            }
            work.push_expr_path(&mut results, path, "infallible byte call path")?;
            continue;
        }
        let mut failure = work.clone_expr_path(&path, "call failure path clone")?;
        let failure_source = work.clone_owned(&source, "call failure status-source clone")?;
        work.push_observation(
            &mut failure,
            SkeletonObservation::Status {
                source: failure_source,
                success: false,
            },
            "call failure observation",
        )?;
        failure.failed = true;
        failure.owned_source = None;
        work.push_expr_path(&mut results, failure, "call failure path")?;

        let success_source = work.clone_owned(&source, "call success status-source clone")?;
        work.push_observation(
            &mut path,
            SkeletonObservation::Status {
                source: success_source,
                success: true,
            },
            "call success observation",
        )?;
        if let Some(source) = allocation_paths::source(program, function, expression, work)? {
            allocation_paths::split_call(&mut results, &mut path, &source, work)?;
        }
        if defer_commit {
            let call = work.clone_owned(&expression.id, "call-commit identity clone")?;
            work.push_observation(
                &mut path,
                SkeletonObservation::CallCommit {
                    call,
                    arguments: commits,
                },
                "deferred success call-commit observation",
            )?;
        }
        if expression.ownership == OwnershipMode::Own
            && type_needs_drop(program, function, &expression.ty)?
        {
            let destination = temporary_place(expression, work)?;
            let at = work.clone_owned(&expression.id, "call result expression clone")?;
            let initialized = work.clone_owned(&destination, "call result destination clone")?;
            work.push_observation(
                &mut path,
                SkeletonObservation::Initialize {
                    at,
                    destination: initialized,
                },
                "owned call result initialization",
            )?;
            path.owned_source = Some(destination);
        } else {
            path.owned_source = None;
        }
        work.push_expr_path(&mut results, path, "call success path")?;
    }
    Ok(results)
}
