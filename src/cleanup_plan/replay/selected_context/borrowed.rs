//! Guarded complete-profile lease. Production Context keeps its old route.
use super::*;
pub(in crate::cleanup_plan::replay) struct Context<'a> {
    source: &'a dyn ReplayContext,
    profile: schema::borrowed::Profile<'a>,
}
impl<'a> Context<'a> {
    pub(in crate::cleanup_plan::replay) fn new(
        source: &'a dyn ReplayContext,
        function: &'a ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<Self, Diagnostic> {
        let profile = schema::borrowed::build(source, function, budget)?;
        let selected = profile.schema(source, function, budget)?;
        // Current typed facts and independently rebuilt inventory are authority;
        // the attached plan/inventory must agree before lending the renewal lease.
        budget.charge(function, 1, "borrowed lease attached schema header")?;
        if !schema::borrowed::same_schema(function, selected, function.cleanup_plan.schema, budget)?
        {
            return Err(replay_error(
                function,
                "borrowed lease cleanup schema disagrees",
            ));
        }
        profile.validate_inventory(source, function, budget)?;
        budget.reserve_array::<Self>(function, 1, "guarded borrowed replay lease")?;
        Ok(Self { source, profile })
    }
}

impl ReplayContext for Context<'_> {
    fn declarations(&self) -> &crate::hir::DeclarationIndex {
        self.source.declarations()
    }
    fn types(&self) -> &[crate::hir::ResolvedTypeDeclaration] {
        self.source.types()
    }
    fn interfaces(&self) -> &[crate::hir::ResolvedInterface] {
        self.source.interfaces()
    }
    fn functions(&self) -> &[ResolvedFunction] {
        self.source.functions()
    }
    fn function_count(&self) -> usize {
        self.source.function_count()
    }
    fn function_at(&self, index: usize) -> Option<&ResolvedFunction> {
        self.source.function_at(index)
    }
    fn function_templates(&self) -> &[crate::hir::ResolvedFunctionTemplate] {
        self.source.function_templates()
    }
    fn function_instances(&self) -> &[crate::hir::ResolvedFunctionInstance] {
        self.source.function_instances()
    }
    fn resolve_call_target(
        &self,
        callee: &crate::hir::DeclarationId,
        instance: Option<&crate::hir::FunctionInstanceId>,
    ) -> Option<&ResolvedFunction> {
        self.source.resolve_call_target(callee, instance)
    }
    fn selected_flat_records(&self) -> bool {
        true
    }
    fn primitive_renewals<'a>(
        &'a self,
        function: &ResolvedFunction,
    ) -> Option<&'a crate::cleanup_plan::ReplayRenewalBindings<'a>> {
        self.profile.renewals(self.source, function)
    }
}
