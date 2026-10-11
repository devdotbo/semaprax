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
    fn borrowed_helper_lease<'a>(
        &'a self,
        function: &ResolvedFunction,
    ) -> Option<crate::cleanup_plan::ReplayBorrowedLease<'a>> {
        self.profile
            .matches(self.source, function)
            .then_some(Lease {
                context: self,
                source: self.source,
                profile: &self.profile,
            })
    }
    fn primitive_renewals<'a>(
        &'a self,
        function: &ResolvedFunction,
    ) -> Option<&'a crate::cleanup_plan::ReplayRenewalBindings<'a>> {
        self.profile.renewals(self.source, function)
    }
}

/// Opaque current-context receiver. Only this private Context can mint it;
/// forwarding a lease through another provider fails its context pointer check.
pub(crate) struct Lease<'a> {
    context: &'a dyn ReplayContext,
    source: &'a dyn ReplayContext,
    profile: &'a schema::borrowed::Profile<'a>,
}
impl Lease<'_> {
    pub(in crate::cleanup_plan::replay) fn validate(
        &self,
        context: &dyn ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        budget.charge(function, 1, "borrowed lease exact receiving context")?;
        if !std::ptr::eq(self.context, context) || !self.profile.matches(self.source, function) {
            return Err(replay_error(
                function,
                "borrowed profile belongs to another receiving context",
            ));
        }
        Ok(())
    }
    pub(in crate::cleanup_plan::replay) fn schema(
        &self,
        context: &dyn ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<&'static str, Diagnostic> {
        self.validate(context, function, budget)?;
        self.profile.schema(self.source, function, budget)
    }
    pub(in crate::cleanup_plan::replay) fn no_drop(
        &self,
        context: &dyn ReplayContext,
        function: &ResolvedFunction,
        ty: &ResolvedType,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        self.validate(context, function, budget)?;
        self.profile.no_drop(self.source, function, ty, budget)
    }
    pub(in crate::cleanup_plan::replay) fn match_shape(
        &self,
        context: &dyn ReplayContext,
        function: &ResolvedFunction,
        expression: &ResolvedExpr,
        budget: &mut ReplayBudget,
    ) -> Result<bool, Diagnostic> {
        self.validate(context, function, budget)?;
        self.profile
            .match_shape(self.source, function, expression, budget)
    }
    pub(in crate::cleanup_plan::replay) fn inventory(
        &self,
        context: &dyn ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        self.validate(context, function, budget)?;
        self.profile
            .validate_inventory(self.source, function, budget)?;
        budget.charge(function, 1, "borrowed plan empty ownership headers")?;
        let plan = &function.cleanup_plan;
        if !plan.slots.is_empty()
            || !plan.flags.is_empty()
            || !plan.entry_state.live_owned_parameters.is_empty()
            || !plan.entry_state.conditional_owned_parameters.is_empty()
        {
            return Err(replay_error(
                function,
                "nonowning helper plan claims owned storage",
            ));
        }
        Ok(())
    }
}
