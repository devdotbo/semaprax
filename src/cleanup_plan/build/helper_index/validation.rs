//! Exact selective registered validation, without construction's eager initializer.
use super::*;
use crate::loop_calls::byte_writer::validation_frontier::{Current, Ordinary};
impl<'a> PlanBuilder<'a> {
    fn with_validation<'f: 'a, 'p: 'a>(
        program: &'a ResolvedProgram,
        function: &'a ResolvedFunction,
        current: &'a Current<'f, 'p>,
    ) -> Result<Self, Diagnostic> {
        if !current.matches(program, function) {
            return Err(plan_error(
                "validation frontier has no exact current member authority",
            ));
        }
        let sites = current.sites()?;
        let sites = HelperSites::Validation {
            sites,
            #[cfg(test)]
            current,
        };
        let mut builder = Self::with_helper_sites(
            program,
            function,
            sites,
            Some(OrdinaryView::Validation(current)),
        )?;
        builder.ordinary_index = Some(OrdinaryView::Validation(current));
        current.complete()?;
        Ok(builder)
    }
}
pub(in crate::cleanup_plan) fn build_current(
    program: &ResolvedProgram,
    current: &Current<'_, '_>,
) -> Result<CleanupPlan, Diagnostic> {
    build_exact(program, current.function(), current)
}
fn build_exact<'f, 'p>(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    current: &Current<'f, 'p>,
) -> Result<CleanupPlan, Diagnostic> {
    let mut builder = PlanBuilder::with_validation(program, function, current)?;
    // Only the old lexical branch has this third selective requires probe.
    // The nonlexical registered route goes straight from initializer to build.
    if let Some(owner) = current.owner() {
        if crate::hir::lexical_exit::requires(function)? {
            super::super::lexical_control::validation::install(&mut builder, owner);
        }
    }
    let plan = builder.build()?;
    current.complete()?;
    Ok(plan)
}
#[cfg(test)]
pub(in crate::cleanup_plan) fn reject_foreign(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    current: &Current<'_, '_>,
) -> Result<CleanupPlan, Diagnostic> {
    build_exact(program, function, current)
}
