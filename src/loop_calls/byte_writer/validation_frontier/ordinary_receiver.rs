//! Independent ordinary facts forced only when the current builder consumes them.
use super::*;
impl<'f, 'p> Current<'f, 'p> {
    fn directory(&self, family: Family) -> Result<&Directory<'p>, ordinary::Refusal> {
        if self.ordinal != self.frontier.next.get() {
            return Err(ordinary::Refusal::Subject);
        }
        if self.frontier.failed.get() {
            return Err(ordinary::Refusal::Work);
        }
        let index = family as usize;
        let cell = self
            .row
            .families
            .get(index)
            .ok_or(ordinary::Refusal::Subject)?;
        if cell.get().is_none() {
            let mut work = |units| self.frontier.units(units);
            let mut backing = crate::bounded_output::reserve_active_required;
            let mut admission = Admission::new(&mut work, &mut backing);
            if matches!(family, Family::Vec | Family::Iterator) {
                if self.frontier.probes.get().is_none() {
                    let probes = crate::hir::renewal_probes::phase::Probes::new(
                        &self.frontier.program.declarations,
                        &mut admission,
                    )?;
                    self.frontier
                        .probes
                        .set(probes)
                        .map_err(|_| ordinary::Refusal::Subject)?;
                }
                admission.probes = self
                    .frontier
                    .probes
                    .get()
                    .map(|probes| probes as &dyn crate::hir::renewal_probes::Index);
            }
            let member = self.row.member;
            let directory = Directory::collect_validation_family(member, family, &mut admission)?;
            cell.set(directory)
                .map_err(|_| ordinary::Refusal::Subject)?;
        }
        cell.get().ok_or(ordinary::Refusal::Subject)
    }
    fn guarded<T>(
        &self,
        operation: impl FnOnce(&mut Admission<'_>) -> Result<T, ordinary::Refusal>,
    ) -> Option<T> {
        if self.frontier.failed.get() {
            return None;
        }
        let mut work = |units| self.frontier.units(units);
        let mut backing = crate::bounded_output::reserve_active_required;
        let mut admission = Admission::new(&mut work, &mut backing);
        match operation(&mut admission) {
            Ok(value) => Some(value),
            Err(_) => {
                self.frontier.failed.set(true);
                None
            }
        }
    }
}
impl<'a, 'f: 'a, 'p: 'a> Ordinary<'a> for Current<'f, 'p> {
    fn matches(&self, program: &ResolvedProgram, function: &ResolvedFunction) -> bool {
        std::ptr::eq(self.frontier.program, program)
            && std::ptr::eq(self.function(), function)
            && self.ordinal == self.frontier.next.get()
    }
    fn has(&self, family: Family) -> bool {
        let directory = match self.directory(family) {
            Ok(directory) => directory,
            Err(_) => {
                self.frontier.failed.set(true);
                return false;
            }
        };
        self.guarded(|admission| directory.has_family(family, admission))
            .unwrap_or(false)
    }
    fn binding(&self, at: &ExpressionId) -> Option<&'a ResolvedBinding> {
        let sites = match self.sites() {
            Ok(sites) => sites,
            Err(_) => {
                self.frontier.failed.set(true);
                return None;
            }
        };
        if let Some(binding) = sites.get(at) {
            return Some(binding);
        }
        for family in [
            Family::String,
            Family::ByteSet,
            Family::Vec,
            Family::Iterator,
        ] {
            let directory = match self.directory(family) {
                Ok(directory) => directory,
                Err(_) => {
                    self.frontier.failed.set(true);
                    return None;
                }
            };
            if let Some((binding, _)) = self.guarded(|admission| {
                directory.receive(self.frontier.program, self.function(), at, admission)
            })? {
                return Some(binding);
            }
        }
        None
    }
    fn complete(&self) -> Result<(), Diagnostic> {
        if self.frontier.failed.get() {
            Err(error())
        } else {
            Ok(())
        }
    }
    #[cfg(test)]
    fn owned_capacity_bytes(&self) -> usize {
        self.frontier.owned_capacity_bytes()
    }
}
