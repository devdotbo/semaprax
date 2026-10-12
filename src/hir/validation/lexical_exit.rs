//! Bottom joins retain only reachable normal ownership states.
use super::*;
impl HirValidator<'_> {
    pub(super) fn validate_lexical_return_operand(
        &self,
        operand: &ResolvedExpr,
        scope: &mut dyn ScopeWrite,
    ) -> Result<(), Diagnostic> {
        if operand.ty == ResolvedType::Never
            || operand.ownership != self.expected_ownership(&operand.ty, OwnershipMode::Own)?
        {
            return Err(hir_error(
                "lexical return operand has invalid result ownership",
            ));
        }
        if operand.ownership == OwnershipMode::Own {
            self.mark_value_sources_moved(operand, scope)?;
        }
        Ok(())
    }
    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub(super) fn finish_lexical_if(
        &self,
        expression: &ResolvedExpr,
        left: &ResolvedExpr,
        right: &ResolvedExpr,
        outer: &mut BTreeMap<ValueId, ValidationBinding>,
        left_scope: &dyn ScopeRead,
        right_scope: &dyn ScopeRead,
        ids: &[ValueId],
    ) -> Result<(), Diagnostic> {
        match (
            super::super::lexical_exit::returns_only(left),
            super::super::lexical_exit::returns_only(right),
        ) {
            (true, false) => Self::merge_availability(outer, right_scope, ids),
            (false, true) => Self::merge_availability(outer, left_scope, ids),
            _ => Self::join_branches(outer, left_scope, right_scope, ids),
        }
        let normal = match (
            left.ty == ResolvedType::Never,
            right.ty == ResolvedType::Never,
        ) {
            (true, false) => right,
            (false, true) => left,
            (a, b) => {
                if !a && !b {
                    self.require_type(&left.ty, &right.ty, "if branches")?;
                    if left.ownership != right.ownership {
                        return Err(hir_error("if branches have inconsistent ownership"));
                    }
                }
                left
            }
        };
        self.finish_expr(expression, &normal.ty, normal.ownership)
    }
    #[allow(clippy::too_many_arguments)]
    pub(super) fn finish_lexical_if_checkpoint(
        &self,
        expression: &ResolvedExpr,
        left: &ResolvedExpr,
        right: &ResolvedExpr,
        outer: &mut dyn ScopeWrite,
        left_scope: &dyn ScopeRead,
        right_scope: &dyn ScopeRead,
        ids: &dyn scope_checkpoint::Ids,
    ) -> Result<(), Diagnostic> {
        match (
            super::super::lexical_exit::returns_only(left),
            super::super::lexical_exit::returns_only(right),
        ) {
            (true, false) => scope_checkpoint::merge::availability(outer, right_scope, ids)?,
            (false, true) => scope_checkpoint::merge::availability(outer, left_scope, ids)?,
            _ => scope_checkpoint::merge::branches(outer, left_scope, right_scope, ids)?,
        }
        let normal = match (
            left.ty == ResolvedType::Never,
            right.ty == ResolvedType::Never,
        ) {
            (true, false) => right,
            (false, true) => left,
            (a, b) => {
                if !a && !b {
                    self.require_type(&left.ty, &right.ty, "if branches")?;
                    if left.ownership != right.ownership {
                        return Err(hir_error("if branches have inconsistent ownership"));
                    }
                }
                left
            }
        };
        self.finish_expr(expression, &normal.ty, normal.ownership)
    }
}

impl HirValidator<'_> {
    pub(super) fn join_lexical_match(
        &self,
        result: &mut Option<(ResolvedType, OwnershipMode)>,
        value: &ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        if let Some((ty, ownership)) = result.as_ref() {
            if *ty != ResolvedType::Never && value.ty != ResolvedType::Never {
                self.require_type(&value.ty, ty, "match arm")?;
                if value.ownership != *ownership {
                    return Err(hir_error("resolved match arms have inconsistent ownership"));
                }
            }
        }
        if result
            .as_ref()
            .is_none_or(|(ty, _)| *ty == ResolvedType::Never)
        {
            *result = Some((value.ty.clone(), value.ownership));
        }
        Ok(())
    }
}
impl HirValidator<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn validate_template_lexical_exit<'value>(
        &mut self,
        template: &'value ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
        expression: &'value ResolvedExpr,
        value: Option<&'value ResolvedExpr>,
        values: &mut generic_template::values::Values<'value>,
        path: &str,
    ) -> Result<(), Diagnostic> {
        if expression.ty != ResolvedType::Never || expression.ownership != OwnershipMode::Value {
            return Err(hir_error("generic lexical exit has a runtime value"));
        }
        if let Some(operand) = value {
            self.validate_template_expr(
                template,
                execution,
                operand,
                values,
                &format!("{path}.value"),
            )?;
            self.require_type(&operand.ty, &template.return_type, "generic lexical return")?;
        }
        // Canonical template replay checks exact target/contour; each materialized
        // instance independently validates its execution-rekeyed typed contour.
        Ok(())
    }
}
