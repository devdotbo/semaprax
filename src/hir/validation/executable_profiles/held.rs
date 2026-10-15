use super::super::call_parameters::ParameterView;
use super::super::held_profiles::{Bound, Call, Key};
use super::*;

impl<'a> Lookup<'a> {
    /// Builds through `new`, then binds privately before anyone can clone it.
    pub(in crate::hir::validation) fn for_program(
        program: &'a ResolvedProgram,
    ) -> Result<Self, Diagnostic> {
        let lookup = Self::new(&program.declarations)?;
        lookup.state.borrow_mut().bound = Some(Bound::new(program));
        Ok(lookup)
    }

    pub(in crate::hir::validation) fn is_bound(&self) -> bool {
        self.state.borrow().bound.is_some()
    }

    /// `resolve_call_target` as a sealed call; `None` when unbound or unresolved.
    pub(in crate::hir::validation) fn call(
        &self,
        callee: &DeclarationId,
        instance: Option<&FunctionInstanceId>,
    ) -> Option<Call<'a>> {
        self.state.borrow().bound.as_ref()?.call(callee, instance)
    }

    /// Known record, non-empty arguments matching its declared parameters.
    fn generic_record(&self, ty: &ResolvedType) -> bool {
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return false;
        };
        if arguments.is_empty() {
            return false;
        }
        let Some(item) = self.index.declaration(declaration) else {
            return false;
        };
        item.kind == DeclarationKind::Record
            && item.id == *declaration
            && self
                .index
                .type_parameters(declaration)
                .is_some_and(|parameters| parameters.len() == arguments.len())
    }

    /// The ownership runtime observation. Anything not an authenticated held
    /// generic record takes the complete old `runtime_admitted` route.
    pub(in crate::hir::validation) fn runtime_admitted_param(
        &self,
        param: &ParameterView<'_>,
    ) -> Result<bool, Diagnostic> {
        paid::fund(Some(0))?;
        let Some(carrier) = param.held() else {
            return self.runtime_admitted(param.ty);
        };
        if !self.generic_record(param.ty) {
            return self.runtime_admitted(param.ty);
        }
        // The RefCell borrow ends before any classifier runs.
        let held: Option<(Key, Option<bool>)> = {
            let state = self.state.borrow();
            state.bound.as_ref().and_then(|bound| {
                let key = bound.authenticate(&carrier, param.ty)?;
                Some((key, bound.recall(key)))
            })
        };
        let Some((key, known)) = held else {
            return self.runtime_admitted(param.ty);
        };
        if let Some(value) = known {
            return Ok(value);
        }
        let value = self.compute(param.ty, Observation::Runtime)?;
        // Unknown, malformed and outside-profile trees retain the complete
        // uncached route. Only the full admitted observation is remembered.
        if !value {
            if let Some(bound) = self.state.borrow_mut().bound.as_mut() {
                bound.settle_runtime_false(key);
            }
            return Ok(false);
        }
        if let Some(bound) = self.state.borrow_mut().bound.as_mut() {
            bound.remember(key, value)?;
        }
        Ok(value)
    }
    #[cfg(test)]
    pub(in crate::hir::validation) fn held_entries(&self) -> usize {
        self.state
            .borrow()
            .bound
            .as_ref()
            .map_or(0, |bound| bound.len())
    }
    /// The first ownership copy observation. Only an authenticated held
    /// generic record consults or fills the memo; every other shape runs the
    /// original `compute` unchanged. `None` and `Err` are never cached.
    pub(in crate::hir::validation) fn copy_for_param(
        &self,
        param: &ParameterView<'_>,
        compute: impl FnOnce(&ResolvedType) -> Result<Option<bool>, Diagnostic>,
    ) -> Result<Option<bool>, Diagnostic> {
        super::super::ownership_flags::Payer::authenticate()?;
        let Some(carrier) = param.held() else {
            return compute(param.ty);
        };
        if !self.generic_record(param.ty) {
            return compute(param.ty);
        }
        // The RefCell borrow ends before the closure runs.
        let held: Option<(Key, Option<bool>)> = {
            let state = self.state.borrow();
            state.bound.as_ref().and_then(|bound| {
                let key = bound.authenticate(&carrier, param.ty)?;
                Some((key, bound.recall_copy(key)))
            })
        };
        let Some((key, known)) = held else {
            return compute(param.ty);
        };
        if let Some(copy) = known {
            return Ok(Some(copy));
        }
        let value = compute(param.ty)?;
        if let Some(copy) = value {
            if let Some(bound) = self.state.borrow_mut().bound.as_mut() {
                bound.remember_copy(key, copy)?;
            }
        }
        Ok(value)
    }
}
