use super::*;

pub(super) fn validate_function(
    program: &ResolvedProgram,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    if helpers.is_none() {
        crate::loop_calls::byte_writer::checked_bindings(program, function)?;
    }
    if crate::hir::iterator_loop::function_has_record_renewal_outside_loop(program, function) {
        return Err(hir_error(
            "record owner renewal is admitted only inside a bounded while body",
        ));
    }
    Ok(())
}

pub(super) fn assignment_is_admitted(
    program: &ResolvedProgram,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    binding: &ResolvedBinding,
    assigned: &ResolvedExpr,
) -> bool {
    helpers.map_or_else(
        || crate::loop_calls::byte_writer::assignment(program, binding, assigned),
        |index| index.admits(binding, assigned),
    ) || crate::vec_ops::is_same_owner_reassignment_hir(program, assigned, &binding.id)
        || crate::stdin_stream_ops::hir_reopen(assigned, &binding.id)
        || crate::byte_ops::is_same_owner_set_hir(assigned, &binding.id)
        || crate::string_ops::is_same_owner_concat_hir(assigned, &binding.id)
        || crate::hir::iterator_loop::is_step_reassignment(assigned, &binding.id)
        || crate::hir::iterator_loop::is_record_owner_renewal(program, binding, assigned)
}

impl<'a> HirValidator<'a> {
    pub(super) fn with_helpers(
        mut self,
        helpers: &'a crate::loop_calls::byte_writer::ModuleIndex<'a>,
    ) -> Self {
        self.byte_helpers = Some(helpers);
        self
    }
}

/// Validate resolved meaning without consulting attached cleanup metadata.
/// Independent cleanup-plan replayers use this boundary to avoid circularly
/// trusting the canonical cleanup-plan builder as their oracle.
pub(crate) fn validate_core(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    validate_with_static_text(program).map(|_| ())
}

pub(crate) fn validate_with_static_text(program: &ResolvedProgram) -> Result<bool, Diagnostic> {
    validate_selected_with_static_text(program, None)
}

/// The same validator, with an explicit joined selection only on the private
/// selected factory path.
pub(crate) fn validate_selected_with_static_text(
    program: &ResolvedProgram,
    fused: Option<crate::hir::selected_fused_candidate::Selected<'_>>,
) -> Result<bool, Diagnostic> {
    let helpers = crate::loop_calls::byte_writer::ModuleIndex::new(program)?;
    HirValidator::new(program)?
        .with_helpers(&helpers)
        .with_selected(fused)
        .validate()
}
