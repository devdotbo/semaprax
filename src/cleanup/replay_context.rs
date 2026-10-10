//! Borrowed typed-HIR authority used by independent cleanup replay.
//! A context lends current declarations and bodies; it never lends a verdict.
use crate::hir::*;

pub(crate) trait ReplayContext {
    fn declarations(&self) -> &DeclarationIndex;
    fn types(&self) -> &[ResolvedTypeDeclaration];
    fn interfaces(&self) -> &[ResolvedInterface];
    fn functions(&self) -> &[ResolvedFunction];
    fn function_templates(&self) -> &[ResolvedFunctionTemplate];
    fn function_instances(&self) -> &[ResolvedFunctionInstance];
    /// Only the selected immutable replay lease implements this method. It
    /// lends freshly rederived, budgeted sites for one exact held function;
    /// ordinary and detached contexts never lend attached-plan authority.
    fn primitive_renewals<'a>(
        &'a self,
        _function: &ResolvedFunction,
    ) -> Option<&'a crate::cleanup_plan::ReplayRenewalBindings<'a>> {
        None
    }
    fn full_program(&self) -> Option<&ResolvedProgram> {
        None
    }
    fn resolve_call_target(
        &self,
        callee: &DeclarationId,
        instance: Option<&FunctionInstanceId>,
    ) -> Option<&ResolvedFunction> {
        match instance {
            None => self
                .functions()
                .iter()
                .find(|function| function.id == *callee),
            Some(id) => self
                .function_instances()
                .iter()
                .find(|candidate| candidate.id == *id && candidate.template == *callee)
                .map(|candidate| &candidate.function),
        }
    }
}
impl ReplayContext for ResolvedProgram {
    fn declarations(&self) -> &DeclarationIndex {
        &self.declarations
    }
    fn types(&self) -> &[ResolvedTypeDeclaration] {
        &self.types
    }
    fn interfaces(&self) -> &[ResolvedInterface] {
        &self.interfaces
    }
    fn functions(&self) -> &[ResolvedFunction] {
        &self.functions
    }
    fn function_templates(&self) -> &[ResolvedFunctionTemplate] {
        &self.function_templates
    }
    fn function_instances(&self) -> &[ResolvedFunctionInstance] {
        &self.function_instances
    }
    fn full_program(&self) -> Option<&ResolvedProgram> {
        Some(self)
    }
}
