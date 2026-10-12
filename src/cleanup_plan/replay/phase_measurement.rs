//! Same global replay budget and inventory; only helper proof reuse differs.
use super::*;
pub(in crate::cleanup_plan) fn replay(
    program: &ResolvedProgram,
    indexed: bool,
) -> Result<(usize, usize), Diagnostic> {
    let mut budget = ReplayBudget::new();
    let helpers = if indexed {
        Some(ordinary_index(program, &mut budget)?)
    } else {
        None
    };
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

/// The actual full-program replay owns its directory independently and debits
/// the same remaining replay ledger before each discovery activity.
pub(super) fn ordinary_index<'a>(
    program: &'a ResolvedProgram,
    budget: &mut ReplayBudget,
) -> Result<crate::loop_calls::byte_writer::ModuleIndex<'a>, Diagnostic> {
    let first = program
        .functions
        .iter()
        .chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        )
        .next();
    let Some(first) = first else {
        return crate::loop_calls::byte_writer::ModuleIndex::new(program);
    };
    let mut refusal = None;
    let result = crate::loop_calls::byte_writer::ModuleIndex::new_with_ordinary_work(
        program,
        None,
        &mut |units| match budget.charge(first, units, "ordinary renewal directory construction") {
            Ok(()) => true,
            Err(error) => {
                refusal = Some(error);
                false
            }
        },
    );
    match result {
        Ok(index) => Ok(index),
        Err(error) => Err(refusal.unwrap_or(error)),
    }
}
