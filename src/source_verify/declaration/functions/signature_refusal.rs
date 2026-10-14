//! A generic profile cannot classify the shape of a missing type component.
//! The ordinary declared-type owner still publishes the authoritative receipt
//! in body preparation. This scratch result neither admits source nor erases
//! independent profile errors on other parameters.
use crate::ast::program_source::ProgramSource;
use crate::ast::{Span, Type};
use crate::source_verify::type_table::TypeTable;
use std::collections::HashSet;

pub(super) fn unknown_component(
    program: &dyn ProgramSource,
    ty: &Type,
    span: Span,
    types: &TypeTable<'_>,
    parameters: &HashSet<&str>,
) -> bool {
    let mut scratch = Vec::new();
    crate::source_verify::declared_type::check_declared_type(
        program,
        ty,
        span,
        types,
        parameters,
        &mut scratch,
    );
    scratch.iter().any(|diagnostic| {
        diagnostic.code == "SPX-T001"
            || diagnostic.code == "SPX-T220"
                && diagnostic
                    .message
                    .ends_with("` is not an in-scope type parameter")
    })
}

/// Borrowed origin issued only inside the public function-body producer.
/// Its private fields bind the exact immutable source and specialization;
/// equality of IDs or spans alone cannot construct a receiving origin.
pub(in crate::source_verify) struct SignatureOrigin<'p> {
    subject: crate::ast::program_source::Subject<'p>,
    template: &'p crate::ast::Function,
    current: &'p crate::ast::Function,
}

pub(super) fn origin<'p>(
    program: &'p dyn ProgramSource,
    template: &'p crate::ast::Function,
    current: &'p crate::ast::Function,
) -> Option<SignatureOrigin<'p>> {
    // Ordinary, un-specialized bodies already use their own exact environment.
    if std::ptr::eq(template, current) {
        return None;
    }
    if !program
        .functions()
        .iter()
        .any(|source| std::ptr::eq(source, template))
        || !(1..=2).contains(&template.type_parameters.len())
        || template
            .type_parameters
            .iter()
            .any(|parameter| !crate::source_verify::diagnostics::source_identifier(&parameter.name))
        || template.type_parameters.len() == 2
            && template.type_parameters[0].name == template.type_parameters[1].name
        || !current.type_parameters.is_empty()
        || template.stable_id != current.stable_id
        || template.name != current.name
        || template.span != current.span
        || template.name_span != current.name_span
        || template.params.len() != current.params.len()
        || !template
            .params
            .iter()
            .zip(&current.params)
            .all(|(raw, concrete)| {
                raw.name == concrete.name && raw.mode == concrete.mode && raw.span == concrete.span
            })
    {
        return None;
    }
    Some(SignatureOrigin {
        subject: program.subject(),
        template,
        current,
    })
}

impl<'p> SignatureOrigin<'p> {
    pub(in crate::source_verify) fn template_for(
        &self,
        program: &dyn ProgramSource,
        current: &crate::ast::Function,
    ) -> Option<&'p crate::ast::Function> {
        (self.subject.same(program.subject()) && std::ptr::eq(current, self.current))
            .then_some(self.template)
    }
}

#[cfg(test)]
mod tests;
