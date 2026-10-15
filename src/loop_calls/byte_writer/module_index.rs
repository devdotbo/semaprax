//! Phase-owned authentication over an immutable typed program. No attached plan
//! or revision-global cache supplies authority; every new phase builds its own.
use super::*;
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedProgram;
mod directory;
mod ordinary;
mod textual;
use directory::Site;
type Sites<'a> = (&'a ResolvedFunction, HelperBindings<'a>);
#[derive(Debug, Default)]
pub(crate) struct ModuleIndex<'a> {
    program: Option<&'a ResolvedProgram>,
    ordinary: Vec<crate::cleanup_plan::renewal_profile::ordinary::Directory<'a>>,
    ordinary_positions: Vec<(usize, usize)>,
    lookup_work: std::cell::Cell<usize>,
    ordinary_failed: std::cell::Cell<bool>,
    sites: Vec<Sites<'a>>,
    // Discovery directories never replace exact borrowed identity checks.
    functions: Vec<usize>,
    expressions: Vec<Site<'a>>,
    // Only completed positive proofs become reusable target authority.
    targets: Vec<&'a ResolvedFunction>,
    #[cfg(test)]
    proof_count: usize,
    #[cfg(test)]
    query_probes: std::cell::Cell<usize>,
}
impl<'a> ModuleIndex<'a> {
    fn new_helpers(program: &'a ResolvedProgram) -> Result<Self, Diagnostic> {
        if !resolved::potential(program) {
            return Ok(Self {
                program: Some(program),
                lookup_work: std::cell::Cell::new(WORK),
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
        let names = textual::Functions::new(&program.functions, &mut lookup_work)?;
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
                    let Some(index) = names.get(callee.as_str(), &mut lookup_work)? else {
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
        let expression_count = sites.iter().try_fold(0usize, |count, (_, bindings)| {
            directory::charge(&mut lookup_work)?;
            count.checked_add(bindings.len()).ok_or_else(error)
        })?;
        let mut functions = directory::reserve::<usize>(sites.len())?;
        let mut expressions = directory::reserve::<Site<'a>>(expression_count)?;
        for (index, (_, bindings)) in sites.iter().enumerate() {
            directory::charge(&mut lookup_work)?;
            functions.push(index);
            for (at, binding) in bindings.borrowed_entries() {
                directory::charge(&mut lookup_work)?;
                expressions.push(Site { at, binding });
            }
        }
        directory::sort(
            &mut functions,
            |index| sites[*index].0 as *const _ as usize,
            &mut lookup_work,
        )?;
        directory::sort(
            &mut expressions,
            |site| site.at as *const _ as usize,
            &mut lookup_work,
        )?;
        directory::sort(
            &mut targets,
            |function| *function as *const _ as usize,
            &mut lookup_work,
        )?;
        Ok(Self {
            program: Some(program),
            ordinary: Vec::new(),
            ordinary_positions: Vec::new(),
            lookup_work: std::cell::Cell::new(lookup_work),
            ordinary_failed: std::cell::Cell::new(false),
            sites,
            functions,
            expressions,
            targets,
            #[cfg(test)]
            proof_count: proofs.iter().filter(|proof| proof.is_some()).count(),
            #[cfg(test)]
            query_probes: std::cell::Cell::new(0),
        })
    }
    pub(crate) fn matches_program(&self, program: &ResolvedProgram) -> bool {
        self.program
            .is_some_and(|actual| std::ptr::eq(actual, program))
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.sites.is_empty()
    }
    fn probe(&self) {
        #[cfg(test)]
        self.query_probes.set(
            self.query_probes
                .get()
                .checked_add(1)
                .expect("bounded test query probes"),
        );
    }
    pub(crate) fn admits(&self, binding: &ResolvedBinding, value: &ResolvedExpr) -> bool {
        let key = &value.id as *const _ as usize;
        let start = directory::lower(
            &self.expressions,
            key,
            |site| site.at as *const _ as usize,
            || self.probe(),
        );
        self.expressions[start..]
            .iter()
            .take_while(|site| site.at as *const _ as usize == key)
            .any(|site| std::ptr::eq(site.at, &value.id) && site.binding == binding)
    }
    /// Reuse only a positive proof of this exact held callee. Targets absent
    /// from the assignment census keep the independent proof fallback.
    pub(crate) fn target(&self, target: &ResolvedFunction) -> bool {
        let Some(program) = self.program else {
            return false;
        };
        // An irrelevant module keeps its allocation-free old membership/proof
        // fallback, rather than deriving authority from an absent directory.
        if self.functions.is_empty() {
            return program
                .functions
                .iter()
                .chain(
                    program
                        .function_instances
                        .iter()
                        .map(|instance| &instance.function),
                )
                .any(|function| std::ptr::eq(function, target))
                && resolved_target(program, target);
        }
        if self.function_sites(target).is_none() {
            return false;
        }
        let key = target as *const _ as usize;
        let index = directory::lower(
            &self.targets,
            key,
            |function| *function as *const _ as usize,
            || self.probe(),
        );
        self.targets
            .get(index)
            .is_some_and(|function| std::ptr::eq(*function, target))
            || resolved_target(program, target)
    }
    pub(crate) fn function_sites(
        &self,
        function: &ResolvedFunction,
    ) -> Option<&HelperBindings<'a>> {
        let key = function as *const _ as usize;
        let position = directory::lower(
            &self.functions,
            key,
            |index| self.sites[*index].0 as *const _ as usize,
            || self.probe(),
        );
        let index = *self.functions.get(position)?;
        let (actual, sites) = &self.sites[index];
        std::ptr::eq(*actual, function).then_some(sites)
    }
    pub(crate) fn has_sites(&self, function: &ResolvedFunction) -> bool {
        self.function_sites(function)
            .is_some_and(|sites| !sites.is_empty())
    }
    /// All borrowed identity backing belongs to the immutable program. Include
    /// every retained Vec capacity, including empty function-site carriers.
    #[cfg(test)]
    pub(crate) fn owned_capacity_bytes(&self) -> usize {
        self.ordinary_positions.capacity() * std::mem::size_of::<(usize, usize)>()
            + self.ordinary.capacity()
                * std::mem::size_of::<crate::cleanup_plan::renewal_profile::ordinary::Directory<'_>>(
                )
            + self
                .ordinary
                .iter()
                .map(|directory| directory.backing_bytes())
                .sum::<usize>()
            + self.functions.capacity() * std::mem::size_of::<usize>()
            + self.expressions.capacity() * std::mem::size_of::<Site<'_>>()
            + self.targets.capacity() * std::mem::size_of::<&ResolvedFunction>()
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
