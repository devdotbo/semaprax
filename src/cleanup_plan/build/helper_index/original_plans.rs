//! Complete original constructor and publication receiver, in an independent namespace.
use super::*;

pub(crate) fn build_program_plans(
    program: &ResolvedProgram,
) -> Result<Vec<CleanupPlan>, Diagnostic> {
    #[cfg(test)]
    let _capacity_scope = capacity_observation::Construction::begin();
    if crate::bounded_output::active_refused() {
        return Err(plan_error(
            "cleanup construction budget was already refused",
        ));
    }
    // Delayed publication keeps the old attached inventories alive while every
    // replacement is staged. Charge this additional overlap before any new
    // index/staging/builder allocation. Attached metadata grants no authority.
    let attached = attached_capacity::program(program)?;
    #[cfg(test)]
    capacity_observation::attached(attached);
    if !crate::bounded_output::reserve_active_required(attached) {
        return Err(plan_error(
            "attached cleanup overlap exceeds admitted scratch",
        ));
    }
    #[cfg(test)]
    CONSTRUCTION_STARTED.with(|started| started.set(true));
    let index = ModuleIndex::new(program)?;
    #[cfg(test)]
    capacity_observation::held_index(index.owned_capacity_bytes());
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
    #[cfg(test)]
    capacity_observation::staged(&plans);
    for function in &program.functions {
        let execution = crate::hir::FunctionExecutionId::Monomorphic(function.id.clone());
        plans.push(lexical_control::build_with_index(
            program, function, &execution, &index,
        )?);
        #[cfg(test)]
        capacity_observation::staged(&plans);
    }
    for instance in &program.function_instances {
        let execution = crate::hir::FunctionExecutionId::Generic(instance.id.clone());
        plans.push(lexical_control::build_with_index(
            program,
            &instance.function,
            &execution,
            &index,
        )?);
        #[cfg(test)]
        capacity_observation::staged(&plans);
    }
    // Intrinsic parameter identities use infallible bounded formatting. A
    // refused format returns an empty diagnostic carrier and marks the ledger;
    // it must never turn into successful staged-plan publication.
    if crate::bounded_output::active_refused() {
        return Err(plan_error("cleanup construction exceeded admitted scratch"));
    }
    Ok(plans)
}

/// All physical staging completes before any attached plan is replaced. A
/// failed construction leaves the input inventory unchanged; it grants no
/// authority to an old or invalid attached plan.
pub(crate) fn attach_program_plans(program: &mut ResolvedProgram) -> Result<(), Diagnostic> {
    let plans = build_program_plans(program)?;
    let mut plans = plans.into_iter();
    for function in &mut program.functions {
        function.cleanup_plan = plans.next().expect("complete staged function inventory");
    }
    for instance in &mut program.function_instances {
        instance.function.cleanup_plan = plans.next().expect("complete staged instance inventory");
    }
    debug_assert!(plans.next().is_none());
    Ok(())
}
