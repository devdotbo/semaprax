use super::*;

impl DeclarationIndex {
    pub(in crate::hir) fn recompute_type_facts(&self, ty: &ResolvedType) -> Option<TypeFacts> {
        self.compute_type_facts(ty, &mut BTreeSet::new(), &mut BTreeMap::new())
    }

    pub(in crate::hir) fn from_verified(program: &Program) -> Result<Self, Diagnostic> {
        Self::from_verified_with_prelude(program, crate::prelude::declarations_for_program(program))
    }
}
