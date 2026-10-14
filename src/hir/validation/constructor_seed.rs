//! Exact current constructor subject, authenticated in the original order.
use super::*;

pub(in crate::hir) struct ConstructorSeed<'a> {
    pub(super) program: &'a ResolvedProgram,
    pub(super) functions: function_lookup::Lookup<'a>,
}
impl<'a> ConstructorSeed<'a> {
    pub(in crate::hir) fn authenticate(
        program: &'a ResolvedProgram,
        directory: Option<&'a super::super::function_directory::Directory<'a>>,
    ) -> Result<Self, Diagnostic> {
        validate_nul_free_identities(program)?;
        if crate::bounded_output::active_remaining().is_some() {
            stdin_stream::paid::reject_sealed_escape_paid(program)?;
        } else {
            stdin_stream::reject_sealed_escape(program)?;
        }
        string_intrinsic::reject_reserved_identities(program)?;
        box_intrinsic::reject_reserved_identities(program)?;
        super::super::closure::once::reject_reserved_identities(program)?;
        generic_template::validate_call_graph(program)?;
        for declaration in program.declarations.declarations() {
            if crate::host_io_ops::by_id(declaration.id.as_str()).is_some()
                || crate::command_io_ops::by_id(declaration.id.as_str()).is_some()
            {
                return Err(hir_error(format!(
                    "resolved {:?} declaration `{}` aliases a compiler-owned host I/O operation",
                    declaration.kind, declaration.id
                )));
            }
        }
        let functions = function_lookup::authenticate(program, directory)?;
        for template in &program.function_templates {
            if crate::host_io_ops::by_id(template.id.as_str()).is_some()
                || crate::command_io_ops::by_id(template.id.as_str()).is_some()
            {
                return Err(hir_error(format!(
                    "resolved function template `{}` aliases a compiler-owned host I/O operation",
                    template.id
                )));
            }
        }
        Ok(Self { program, functions })
    }
    pub(in crate::hir) fn authenticate_shared(
        program: &'a ResolvedProgram,
        shared: std::rc::Rc<super::super::function_directory::Directory<'a>>,
    ) -> Result<Self, Diagnostic> {
        validate_nul_free_identities(program)?;
        if crate::bounded_output::active_remaining().is_some() {
            stdin_stream::paid::reject_sealed_escape_paid(program)?;
        } else {
            stdin_stream::reject_sealed_escape(program)?;
        }
        string_intrinsic::reject_reserved_identities(program)?;
        box_intrinsic::reject_reserved_identities(program)?;
        super::super::closure::once::reject_reserved_identities(program)?;
        generic_template::validate_call_graph(program)?;
        for declaration in program.declarations.declarations() {
            if crate::host_io_ops::by_id(declaration.id.as_str()).is_some()
                || crate::command_io_ops::by_id(declaration.id.as_str()).is_some()
            {
                return Err(hir_error(format!(
                    "resolved {:?} declaration `{}` aliases a compiler-owned host I/O operation",
                    declaration.kind, declaration.id
                )));
            }
        }
        let functions = function_lookup::authenticate_shared(program, None, Some(shared))?;
        for template in &program.function_templates {
            if crate::host_io_ops::by_id(template.id.as_str()).is_some()
                || crate::command_io_ops::by_id(template.id.as_str()).is_some()
            {
                return Err(hir_error(format!(
                    "resolved function template `{}` aliases a compiler-owned host I/O operation",
                    template.id
                )));
            }
        }
        Ok(Self { program, functions })
    }
    pub(in crate::hir) fn program(&self) -> &'a ResolvedProgram {
        self.program
    }
}

#[cfg(test)]
pub(super) mod original;
