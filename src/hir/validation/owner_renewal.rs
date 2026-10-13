use super::*;

#[cfg(test)]
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

pub(super) fn validate_function_paid(
    program: &ResolvedProgram,
    helpers: Option<&crate::loop_calls::byte_writer::ModuleIndex<'_>>,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    if helpers.is_none() {
        crate::loop_calls::byte_writer::checked_bindings(program, function)?;
    }
    if crate::hir::iterator_loop::function_has_record_renewal_outside_loop_paid(program, function)?
    {
        return Err(hir_error_static(
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
#[cfg(test)]
pub(crate) fn validate_core(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    validate_with_static_text(program).map(|_| ())
}

#[cfg(test)]
pub(crate) fn validate_with_static_text(program: &ResolvedProgram) -> Result<bool, Diagnostic> {
    validate_selected_with_static_text(program, None)
}

/// The same validator, with an explicit joined selection only on the private
/// selected factory path.
#[cfg(test)]
pub(crate) fn validate_selected_with_static_text(
    program: &ResolvedProgram,
    fused: Option<crate::hir::selected_fused_candidate::Selected<'_>>,
) -> Result<bool, Diagnostic> {
    validate_with_directory(program, fused, None)
}

#[cfg(test)]
pub(in crate::hir) fn validate_with_directory<'a>(
    program: &'a ResolvedProgram,
    fused: Option<crate::hir::selected_fused_candidate::Selected<'a>>,
    directory: Option<&'a crate::hir::function_directory::Directory<'a>>,
) -> Result<bool, Diagnostic> {
    let observation = Observation::default();
    let helpers = crate::loop_calls::byte_writer::ModuleIndex::new(program)?;
    let validator = HirValidator::with_directory(program, directory)?;
    validate_prepared(program, fused, &helpers, validator, &observation)
}

// The stack owner precedes fresh validator construction, so invariant lazy
// lookup state may borrow this primary-walk lifetime from the start.
#[derive(Default)]
pub(in crate::hir) struct Observation {
    row_loans: std::cell::Cell<bool>,
    byte_traversals: std::cell::Cell<bool>,
}

pub(in crate::hir) fn validate_prepared<'a>(
    program: &'a ResolvedProgram,
    fused: Option<crate::hir::selected_fused_candidate::Selected<'a>>,
    helpers: &'a crate::loop_calls::byte_writer::ModuleIndex<'a>,
    validator: HirValidator<'a>,
    observation: &'a Observation,
) -> Result<bool, Diagnostic> {
    let selected = validator
        .with_row_loan_observer(&observation.row_loans)
        .with_byte_traversal_observer(&observation.byte_traversals)
        .with_helpers(helpers)
        .with_selected(fused)
        .validate()?;
    // This observation belongs only to this successful complete primary walk;
    // no attached marker or persisted hint can suppress independent replay.
    if observation.row_loans.get() {
        crate::hir::row_loan::validate_program(program)?;
    }
    if observation.byte_traversals.get() {
        for function in program.functions.iter().chain(
            program
                .function_instances
                .iter()
                .map(|instance| &instance.function),
        ) {
            byte_traversal::validate_function(&program.declarations, function)?;
        }
    }
    Ok(selected)
}

#[cfg(test)]
mod scan_tests {
    use super::*;
    #[test]
    fn outside_record_renewal_retains_complete_old_diagnostic_without_owned_message_backing() {
        let source = r#"module scan.record;
@id("cursor") record Cursor { @id("cursor.bytes") bytes: Bytes, @id("cursor.index") index: usize, }
@id("advance") fn advance(value: own Cursor)->Cursor { value }
@id("main") fn main()->i64 {
 let mut cursor=Cursor{bytes:bytes_zeroed(0usize),index:0usize};
 while false { cursor=advance(cursor); continue; }
 0
}"#;
        let ast = crate::check(source, "outside-record-scan.spx").unwrap();
        let program = crate::hir::resolve(&ast).unwrap();
        let helpers = crate::loop_calls::byte_writer::ModuleIndex::new(&program).unwrap();
        let mut function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "main")
            .unwrap()
            .clone();
        let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
            panic!("block");
        };
        let (ResolvedStatement::While { body, .. } | ResolvedStatement::LexicalWhile { body, .. }) =
            &statements[1]
        else {
            panic!("while");
        };
        let ResolvedExprKind::Block {
            statements: inner, ..
        } = &body.kind
        else {
            panic!("loop block");
        };
        let assignment = inner[0].clone();
        statements.insert(1, assignment);
        let expected = validate_function(&program, Some(&helpers), &function).unwrap_err();
        let ((actual, metrics), refused, paid) = crate::bounded_output::with_limit_usage(0, || {
            crate::test_allocation_observer::measure(|| {
                validate_function_paid(&program, Some(&helpers), &function)
            })
        });
        assert_eq!(actual.unwrap_err(), expected);
        assert!(!refused && !metrics.invalid);
        assert_eq!(paid, 0);
        assert_eq!(metrics.allocations, 0);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
}
