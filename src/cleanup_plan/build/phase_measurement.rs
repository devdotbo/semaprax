//! Private paired construction control. Both routes stage a complete program.
use super::*;
pub(in crate::cleanup_plan) fn construct(
    program: &ResolvedProgram,
    indexed: bool,
) -> Result<Vec<CleanupPlan>, Diagnostic> {
    let attached = helper_index::attached_capacity_for_measurement(program)?;
    if !crate::bounded_output::reserve_active_required(attached) {
        return Err(plan_error(
            "attached cleanup overlap exceeds admitted scratch",
        ));
    }
    let index = if indexed {
        Some(crate::loop_calls::byte_writer::ModuleIndex::new(program)?)
    } else {
        None
    };
    let count = program
        .functions
        .len()
        .checked_add(program.function_instances.len())
        .ok_or_else(|| plan_error("cleanup plan staging capacity overflowed"))?;
    let bytes = count
        .checked_mul(std::mem::size_of::<CleanupPlan>())
        .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Vec<CleanupPlan>>()))
        .ok_or_else(|| plan_error("cleanup plan staging capacity overflowed"))?;
    if !crate::bounded_output::reserve_active_required(bytes) {
        return Err(plan_error("cleanup plan staging exceeds admitted scratch"));
    }
    let mut plans = Vec::new();
    plans
        .try_reserve_exact(count)
        .map_err(|_| plan_error("cleanup plan staging allocation failed"))?;
    if plans.capacity() != count {
        return Err(plan_error(
            "cleanup plan staging backing exceeds reserved capacity",
        ));
    }
    for (function, execution) in program
        .functions
        .iter()
        .map(|function| {
            (
                function,
                crate::hir::FunctionExecutionId::Monomorphic(function.id.clone()),
            )
        })
        .chain(program.function_instances.iter().map(|instance| {
            (
                &instance.function,
                crate::hir::FunctionExecutionId::Generic(instance.id.clone()),
            )
        }))
    {
        plans.push(if let Some(index) = &index {
            lexical_control::build_with_index(program, function, &execution, index)?
        } else {
            lexical_control::build_plan_for_execution(program, function, &execution)?
        });
    }
    Ok(plans)
}
