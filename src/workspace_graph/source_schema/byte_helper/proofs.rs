//! Positive proof reuse belongs only to one held immutable provider inventory.
use super::*;

pub(in crate::workspace_graph) struct Proofs<'a> {
    modules: &'a [WorkspaceResolvedModule],
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    proved: Vec<&'a hir::ResolvedFunction>,
    cursor: Option<Box<crate::loop_calls::byte_writer::ProofCursor<'a>>>,
    work: usize,
    #[cfg(test)]
    pub(super) proof_count: usize,
}
impl<'a> Proofs<'a> {
    pub(in crate::workspace_graph) fn new(
        modules: &'a [WorkspaceResolvedModule],
        facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    ) -> Self {
        Self {
            modules,
            facts,
            proved: Vec::new(),
            cursor: None,
            work: crate::cache_codec::MAX_NODES,
            #[cfg(test)]
            proof_count: 0,
        }
    }
    pub(super) fn prove(
        &mut self,
        modules: &'a [WorkspaceResolvedModule],
        facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
        callee: &'a hir::ResolvedFunction,
        lookup_work: usize,
    ) -> Result<bool, Vec<Diagnostic>> {
        let fail = || {
            vec![graph_error(
                "SPX-G410",
                "retained byte helper renewal authority disagrees",
            )]
        };
        if !std::ptr::eq(self.modules, modules)
            || !std::ptr::eq(self.facts, facts)
            || !target(modules, facts, &callee.id)
                .is_some_and(|(_, retained)| std::ptr::eq(retained, callee))
        {
            return Err(fail());
        }
        self.work = self
            .work
            .checked_sub(self.proved.len().checked_add(1).ok_or_else(fail)?)
            .ok_or_else(fail)?;
        if self
            .proved
            .iter()
            .any(|retained| std::ptr::eq(*retained, callee))
        {
            return Ok(true);
        }
        if self.cursor.is_none() {
            let count = modules
                .iter()
                .try_fold(0usize, |sum, module| {
                    sum.checked_add(module.functions.len())
                })
                .ok_or_else(fail)?;
            let bytes = count
                .checked_mul(std::mem::size_of::<&hir::ResolvedFunction>())
                .and_then(|bytes| bytes.checked_add(std::mem::size_of::<Self>()))
                .ok_or_else(fail)?;
            reserve_builder_structure(bytes)?;
            self.proved.try_reserve_exact(count).map_err(|_| fail())?;
            if self.proved.capacity() != count {
                return Err(fail());
            }
            self.cursor = Some(Box::new(
                crate::loop_calls::byte_writer::ProofCursor::new()
                    .map_err(|_| vec![limit_error("builder_bytes", active_builder_limit())])?,
            ));
        }
        #[cfg(test)]
        {
            self.proof_count += 1;
        }
        let proved = crate::loop_calls::byte_writer::resolved_nonallocating_with_cursor(
            callee,
            |id, instance| {
                if instance.is_none() {
                    target(modules, facts, id).map(|(_, f)| f)
                } else {
                    None
                }
            },
            |function, mode, ty| {
                target(modules, facts, &function.id)
                    .filter(|(_, retained)| std::ptr::eq(*retained, function))
                    .is_some_and(|(owner, _)| {
                        type_authority::parameter(owner, modules, facts, mode, ty)
                    })
            },
            |function, ty| {
                target(modules, facts, &function.id)
                    .filter(|(_, retained)| std::ptr::eq(*retained, function))
                    .is_some_and(|(owner, _)| {
                        *ty == ResolvedType::Bytes
                            || type_authority::copy(owner, modules, facts, ty)
                    })
            },
            lookup_work,
            self.cursor.as_deref_mut().ok_or_else(fail)?,
        );
        if crate::bounded_output::active_refused() {
            return Err(vec![limit_error("builder_bytes", active_builder_limit())]);
        }
        if proved {
            if self.proved.len() == self.proved.capacity() {
                return Err(fail());
            }
            self.proved.push(callee);
        }
        Ok(proved)
    }
}
