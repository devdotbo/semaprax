use super::super::*;
impl HirValidator<'_> {
    pub(in crate::hir::validation) fn validate_closure_owned_reference_body(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &mut dyn ScopeWrite,
        path: &str,
        allow_moves: bool,
    ) -> Result<(), Diagnostic> {
        crate::hir::closure::validate_shape(self.program, expression)?;
        let ResolvedExprKind::Closure { captures, .. } = &expression.kind else {
            unreachable!()
        };
        let affine = expression.ty.is_once_function();
        if affine && !allow_moves {
            return Err(hir_error(
                "affine construction cannot consume in a contract",
            ));
        }
        let ownership = if affine || expression.ty.is_mut_function() {
            OwnershipMode::Own
        } else {
            OwnershipMode::Value
        };
        for (index, capture) in captures.iter().enumerate() {
            let ResolvedExprKind::Place(place) = &capture.value.kind else {
                unreachable!()
            };
            if !capture
                .value
                .id
                .matches(function, &format!("{path}.capture.{index}"))
                || !self.expression_ids.insert(capture.value.id.clone())
            {
                return Err(hir_error(
                    "closure capture expression identity is not canonical",
                ));
            }
            if scope.get(&place.root).is_none_or(|binding| {
                binding.ty != capture.value.ty
                    || binding.ownership != capture.value.ownership
                    || binding.availability != Availability::Available
            }) {
                return Err(hir_error(
                    "closure capture does not name an available outer scalar",
                ));
            }
        }
        if affine {
            for capture in captures
                .iter()
                .filter(|c| c.value.ownership == OwnershipMode::Own)
            {
                self.mark_value_sources_moved(&capture.value, scope)?;
            }
        }
        let body = crate::hir::closure::closure_function(self.program, expression)?;
        let execution = FunctionExecutionId::Monomorphic(body.id.clone());
        let mut checked = if let Some(storage) = self.canonical_loans.shared() {
            let seed = constructor_seed::ConstructorSeed::authenticate(
                self.program,
                self.functions.directory(),
            )?;
            HirValidator::with_storage(seed, storage)?
        } else {
            HirValidator::with_directory(self.program, self.functions.directory())?
        };
        checked.validate_function_owned_reference(&body, &execution)?;
        if checked
            .expression_ids
            .iter()
            .any(|id| !self.expression_ids.insert(id.clone()))
            || checked
                .value_ids
                .iter()
                .any(|id| !self.value_ids.insert(id.clone()))
        {
            return Err(hir_error(
                "closure body identities collide with the enclosing program",
            ));
        }
        self.finish_expr(expression, &expression.ty, ownership)
    }
}

impl HirValidator<'_> {
    pub(in crate::hir::validation) fn validate_template_closure_owned_reference_body<'value>(
        &mut self,
        template: &'value ResolvedFunctionTemplate,
        execution: &FunctionExecutionId,
        expression: &'value ResolvedExpr,
        values: &mut generic_template::values::Values<'value>,
        path: &str,
    ) -> Result<(), Diagnostic> {
        if !crate::hir::generic_collection::profile(template) {
            return Err(hir_error(
                "generic closure requires the private scalar collection profile",
            ));
        }
        crate::hir::closure::validate_shape_scoped(self.program, expression, Some(&template.id))?;
        let ResolvedExprKind::Closure {
            captures,
            parameters,
            body,
        } = &expression.kind
        else {
            unreachable!()
        };
        for (index, capture) in captures.iter().enumerate() {
            self.validate_template_expr(
                template,
                execution,
                &capture.value,
                values,
                &format!("{path}.capture.{index}"),
            )?;
        }
        let mut body_values = values.empty_like()?;
        for binding in captures
            .iter()
            .map(|capture| &capture.binding)
            .chain(parameters.iter())
        {
            if !self.value_ids.insert(binding.id.clone())
                || body_values.insert(binding.id.clone(), &binding.ty)?
            {
                return Err(hir_error("generic closure bindings collide"));
            }
        }
        let body_execution =
            FunctionExecutionId::Monomorphic(crate::hir::closure::closure_id(&expression.id));
        self.validate_template_expr(template, &body_execution, body, &mut body_values, "body")?;
        self.finish_expr(expression, &expression.ty, OwnershipMode::Value)
    }
}

impl HirValidator<'_> {
    pub(in crate::hir::validation) fn finish_affine_invocation_owned_reference(
        &self,
        expression: &ResolvedExpr,
        scope: &mut dyn ScopeWrite,
        allow_moves: bool,
    ) -> Result<(), Diagnostic> {
        if let ResolvedExprKind::Invoke { callable, .. } = &expression.kind {
            if callable.ty.is_mut_function() {
                crate::hir::closure::mutable::validate_receiver(
                    self.program,
                    callable,
                    allow_moves,
                )?;
            }
            if callable.ty.is_once_function() {
                if !allow_moves {
                    return Err(hir_error("affine invocation cannot consume in a contract"));
                }
                self.mark_value_sources_moved(callable, scope)?;
            }
        }
        Ok(())
    }
}
