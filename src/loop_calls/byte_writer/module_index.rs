//! Phase-owned authentication over an immutable typed program. No attached plan
//! or revision-global cache supplies authority; every new phase builds its own.
use super::*;
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
type Sites<'a> = (&'a ResolvedFunction, HelperBindings<'a>);
#[derive(Debug, Default)]
pub(crate) struct ModuleIndex<'a> {
    program: Option<&'a ResolvedProgram>,
    sites: Vec<Sites<'a>>,
    // Only completed positive proofs become reusable target authority.
    targets: Vec<&'a ResolvedFunction>,
    #[cfg(test)]
    proof_count: usize,
}
impl<'a> ModuleIndex<'a> {
    pub(crate) fn new(program: &'a ResolvedProgram) -> Result<Self, Diagnostic> {
        if !resolved::potential(program) {
            return Ok(Self {
                program: Some(program),
                ..Self::default()
            });
        }
        let error = resolved::census_error;
        let count = program
            .functions
            .len()
            .checked_add(program.function_instances.len())
            .ok_or_else(error)?;
        let bytes = count
            .checked_mul(std::mem::size_of::<Sites<'a>>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or_else(error)?;
        let proof_bytes = program
            .functions
            .len()
            .checked_mul(std::mem::size_of::<Option<bool>>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Vec<Option<bool>>>()))
            .ok_or_else(error)?;
        if !crate::bounded_output::reserve_active_required(
            bytes.checked_add(proof_bytes).ok_or_else(error)?,
        ) {
            return Err(error());
        }
        let mut sites = Vec::new();
        sites.try_reserve_exact(count).map_err(|_| error())?;
        let mut proofs = Vec::new();
        proofs
            .try_reserve_exact(program.functions.len())
            .map_err(|_| error())?;
        if sites.capacity() != count || proofs.capacity() != program.functions.len() {
            return Err(error());
        }
        proofs.resize(program.functions.len(), None);
        let Some(first) = program.functions.first() else {
            return Err(error());
        };
        let mut cursor = resolved::CensusCursor::new(first)?;
        let mut proof_cursor = None;
        let mut lookup_work = WORK;
        for function in program
            .functions
            .iter()
            .chain(program.function_instances.iter().map(|i| &i.function))
        {
            let bindings =
                resolved::checked_bindings_with(function, &mut cursor, |binding, value| {
                    let ResolvedExprKind::Call {
                        callee,
                        instance: None,
                        ..
                    } = &value.kind
                    else {
                        return Ok(false);
                    };
                    if binding.ty != ResolvedType::Bytes || binding.ownership != OwnershipMode::Own
                    {
                        return Ok(false);
                    }
                    if crate::byte_ops::by_id(callee.as_str()).is_some() {
                        return Ok(false);
                    }
                    lookup_work = lookup_work
                        .checked_sub(program.functions.len().checked_add(1).ok_or_else(error)?)
                        .ok_or_else(error)?;
                    let Some(index) = program.functions.iter().position(|f| &f.id == callee) else {
                        return Ok(false);
                    };
                    let target = &program.functions[index];
                    let proved = match proofs[index] {
                        Some(proved) => proved,
                        None => {
                            let proved =
                                resolved::checked_target(program, target, &mut proof_cursor)?;
                            proofs[index] = Some(proved);
                            proved
                        }
                    };
                    Ok(assignment_with_target(
                        program, binding, value, target, proved,
                    ))
                })?;
            sites.push((function, bindings));
        }
        let positive = proofs.iter().filter(|proof| **proof == Some(true)).count();
        let target_bytes = positive
            .checked_mul(std::mem::size_of::<&ResolvedFunction>())
            .ok_or_else(error)?;
        if !crate::bounded_output::reserve_active_required(target_bytes) {
            return Err(error());
        }
        let mut targets = Vec::new();
        targets.try_reserve_exact(positive).map_err(|_| error())?;
        if targets.capacity() != positive {
            return Err(error());
        }
        targets.extend(
            program
                .functions
                .iter()
                .zip(&proofs)
                .filter_map(|(function, proof)| (*proof == Some(true)).then_some(function)),
        );
        Ok(Self {
            program: Some(program),
            sites,
            targets,
            #[cfg(test)]
            proof_count: proofs.iter().filter(|proof| proof.is_some()).count(),
        })
    }
    pub(crate) fn matches_program(&self, program: &ResolvedProgram) -> bool {
        self.program
            .is_some_and(|actual| std::ptr::eq(actual, program))
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.sites.is_empty()
    }
    pub(crate) fn admits(&self, binding: &ResolvedBinding, value: &ResolvedExpr) -> bool {
        self.sites.iter().any(|(_, sites)| {
            sites
                .get_exact(&value.id)
                .is_some_and(|actual| actual == binding)
        })
    }
    /// Reuse only a positive proof of this exact held callee. Targets absent
    /// from the assignment census keep the independent proof fallback.
    pub(crate) fn target(&self, target: &ResolvedFunction) -> bool {
        let Some(program) = self.program else {
            return false;
        };
        if !program
            .functions
            .iter()
            .chain(
                program
                    .function_instances
                    .iter()
                    .map(|instance| &instance.function),
            )
            .any(|function| std::ptr::eq(function, target))
        {
            return false;
        }
        self.targets
            .iter()
            .any(|function| std::ptr::eq(*function, target))
            || resolved_target(program, target)
    }
    pub(crate) fn function_sites(
        &self,
        function: &ResolvedFunction,
    ) -> Option<&HelperBindings<'a>> {
        self.sites
            .iter()
            .find(|(f, _)| std::ptr::eq(*f, function))
            .map(|(_, sites)| sites)
    }
    pub(crate) fn has_sites(&self, function: &ResolvedFunction) -> bool {
        self.sites
            .iter()
            .find(|(f, _)| std::ptr::eq(*f, function))
            .is_some_and(|(_, sites)| !sites.is_empty())
    }
    /// All borrowed identity backing belongs to the immutable program. Include
    /// every retained Vec capacity, including empty function-site carriers.
    #[cfg(test)]
    pub(crate) fn owned_capacity_bytes(&self) -> usize {
        self.targets.capacity() * std::mem::size_of::<&ResolvedFunction>()
            + self.sites.capacity() * std::mem::size_of::<Sites<'a>>()
            + self
                .sites
                .iter()
                .map(|(_, sites)| sites.owned_capacity_bytes())
                .sum::<usize>()
    }
}

#[cfg(test)]
mod tests;
