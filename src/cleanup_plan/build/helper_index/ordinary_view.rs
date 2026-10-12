//! Construction retains its completed index; validation lends one sealed demand frontier.
use super::*;
use crate::cleanup_plan::renewal_profile::ordinary::Family;
#[derive(Clone, Copy)]
pub(in crate::cleanup_plan::build) enum OrdinaryView<'a> {
    Construction(&'a ModuleIndex<'a>),
    Validation(&'a dyn crate::loop_calls::byte_writer::validation_frontier::Ordinary<'a>),
}
impl<'a> OrdinaryView<'a> {
    pub(super) fn has(
        self,
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        family: Family,
    ) -> bool {
        match self {
            Self::Construction(index) => index.ordinary_has(program, function, family),
            Self::Validation(current) => current.matches(program, function) && current.has(family),
        }
    }
    pub(super) fn at(
        self,
        program: &ResolvedProgram,
        function: &ResolvedFunction,
        at: &ExpressionId,
    ) -> Option<(&'a ResolvedBinding, u8)> {
        match self {
            Self::Construction(index) => index.ordinary_at(program, function, at),
            Self::Validation(current) => current
                .matches(program, function)
                .then(|| current.binding(at))
                .flatten()
                .map(|binding| (binding, 0)),
        }
    }
}
