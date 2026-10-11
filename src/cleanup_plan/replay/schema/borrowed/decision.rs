//! Current typed-node facts for the guarded complete helper profile.
//! These facts are populated only by the complete contracts/body/signature walk;
//! a renewal lease, plan schema, inventory attachment or nominal spelling cannot
//! populate them. Unsupported owning/aggregate families prevent publication.
use super::*;

#[derive(Default)]
pub(super) struct Facts {
    lexical: bool,
    owning_expression: bool,
    owning_binding: bool,
    aggregate_type: bool,
    record_update: bool,
    record_constructor: bool,
    explicit_variant: bool,
    explicit_record: bool,
    byte_range: bool,
    option_try: bool,
}
impl Facts {
    pub(super) fn ty(&mut self, admitted_copy_or_view: bool) {
        // This input is the actual budgeted type receiver's verdict, never a
        // nominal spelling or attached inventory classification.
        self.aggregate_type |= !admitted_copy_or_view;
    }
    pub(super) fn expression(
        &mut self,
        expression: &ResolvedExpr,
        admitted_type: bool,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<(), Diagnostic> {
        budget.charge(function, 1, "borrowed helper schema expression facts")?;
        self.ty(admitted_type);
        self.owning_expression |= expression.ownership == OwnershipMode::Own;
        self.lexical |= matches!(&expression.kind, ResolvedExprKind::LexicalExit { .. });
        self.record_update |= matches!(&expression.kind, ResolvedExprKind::UpdateRecord { .. });
        self.record_constructor |=
            matches!(&expression.kind, ResolvedExprKind::ConstructRecord { .. });
        self.byte_range |= matches!(&expression.kind, ResolvedExprKind::ByteRange { .. });
        self.option_try |= matches!(&expression.kind, ResolvedExprKind::TryOption { .. });
        if let ResolvedExprKind::Match { mode, arms, .. } = &expression.kind {
            if matches!(
                mode,
                crate::hir::ResolvedMatchMode::Own | crate::hir::ResolvedMatchMode::Borrow
            ) {
                for arm in arms {
                    budget.charge(function, 1, "borrowed helper schema pattern facts")?;
                    self.explicit_variant |=
                        matches!(&arm.pattern, ResolvedMatchPattern::Variant { .. });
                    self.explicit_record |=
                        matches!(&arm.pattern, ResolvedMatchPattern::Record { .. });
                }
            }
        }
        Ok(())
    }
    pub(super) fn binding(&mut self, ownership: OwnershipMode, admitted_type: bool) {
        self.ty(admitted_type);
        self.owning_binding |= ownership == OwnershipMode::Own;
    }
    fn complete_absence(&self) -> bool {
        !self.lexical
            && !self.owning_expression
            && !self.owning_binding
            && !self.aggregate_type
            && !self.record_update
            && !self.record_constructor
    }
}
impl Profile<'_> {
    /// Reuse only the exact current complete proof and rebuilt inventory.
    /// The ordinary selector remains independent and is the test oracle. The
    /// profile proves absence of owning byte/String/Vec/Step bindings, native
    /// owner arguments, iterator types, record constructions/updates and nested
    /// shapes; these are the exact prerequisites of v17 through v7. It does
    /// not infer their absence from the empty renewal lookup.
    pub(super) fn schema(
        &self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
    ) -> Result<&'static str, Diagnostic> {
        if !self.matches(program, function) || !self.schema_facts.complete_absence() {
            return Err(fail(function));
        }
        let inventory = &self.inventory;
        // Empty output follows the independently metered no-drop inventory
        // constructor, rather than any attached inventory or plan. Unexpected
        // storage invalidates the sealed proof instead of guessing its shape.
        if !inventory.slots.is_empty()
            || !inventory.flags.is_empty()
            || !inventory.entry_state.live_owned_parameters.is_empty()
            || !inventory
                .entry_state
                .conditional_owned_parameters
                .is_empty()
        {
            return Err(fail(function));
        }
        let guard = |budget: &mut ReplayBudget, phase| budget.charge(function, 1, phase);
        guard(budget, "schema v19 writer guard")?;
        if self.renewals.has_helpers() {
            return Ok(crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19);
        }
        guard(budget, "schema v18 lexical guard")?;
        if self.schema_facts.lexical {
            return Ok(crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V18);
        }
        // Preserve the ordinary selector's precomputation before its v17
        // chain. Every contributing owning/record/type prerequisite has already
        // been visited and charged in the complete profile. Empty inventory
        // has no recursive shape or lifecycle work to repeat here.
        guard(budget, "schema nested inventory and record precomputation")?;
        for phase in [
            "schema v17 same-owner Bytes guard",
            "schema v16 String replacement guard",
            "schema v15 Vec renewal guard",
            "schema v14 native owner admission guard",
            "schema v13 owned iterator guard",
            "schema v12 iterator renewal guard",
            "schema v11 iterator loop guard",
            "schema v10 iterator lifecycle guard",
            "schema v9 nested record update guard",
            "schema v8 nested record destructure guard",
            "schema v7 nested owned byte guard",
        ] {
            guard(budget, phase)?;
        }
        guard(budget, "schema v6 inventory guard")?;
        if id_equal(
            function,
            inventory.schema,
            crate::cleanup::CLEANUP_INVENTORY_SCHEMA_V2,
            budget,
        )? {
            return Ok(crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V6);
        }
        guard(budget, "schema v6 explicit variant guard")?;
        if self.schema_facts.explicit_variant {
            return Ok(crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V6);
        }
        guard(budget, "schema v5 explicit record guard")?;
        if self.schema_facts.explicit_record {
            return Ok(crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V5);
        }
        guard(budget, "schema v4 byte range guard")?;
        if self.schema_facts.byte_range {
            return Ok(crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V4);
        }
        guard(budget, "schema v3 Option try guard")?;
        if self.schema_facts.option_try {
            return Ok(crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V3);
        }
        Ok(crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V2)
    }
}
