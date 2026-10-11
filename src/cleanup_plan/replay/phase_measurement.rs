//! Same global replay budget and inventory; only helper proof reuse differs.
use super::*;
pub(in crate::cleanup_plan) fn replay(
    program: &ResolvedProgram,
    indexed: bool,
) -> Result<(usize, usize), Diagnostic> {
    let helpers = if indexed {
        Some(crate::loop_calls::byte_writer::ModuleIndex::new(program)?)
    } else {
        None
    };
    let mut budget = ReplayBudget::new();
    reserve_program_skeleton_work(
        program,
        program.functions.iter().chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        ),
        &mut budget,
    )?;
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        validate_structure_with_budget(program, function, &mut budget, helpers.as_ref())?;
    }
    Ok((
        MAX_REPLAY_WORK_UNITS - budget.remaining,
        budget.skeleton_spent,
    ))
}
