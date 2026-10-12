use super::*;

impl DeclarationIndex {
    /// Only construction's later passes use this: the same index has already
    /// inserted every top-level subject (or the immediately preceding method).
    /// Borrow text for lookup and share its exact existing valid carrier. This
    /// does not authenticate a source ID or pay initial legacy construction.
    pub(super) fn inserted_identity(&self, text: &str) -> DeclarationId {
        self.declarations
            .get(text)
            .expect("subject inserted by the preceding index pass")
            .id
            .clone()
    }

    pub(in crate::hir) fn recompute_type_facts(&self, ty: &ResolvedType) -> Option<TypeFacts> {
        self.compute_type_facts(ty, &mut BTreeSet::new(), &mut BTreeMap::new())
    }

    pub(in crate::hir) fn from_verified(program: &Program) -> Result<Self, Diagnostic> {
        Self::from_verified_with_prelude(program, crate::prelude::declarations_for_program(program))
    }
}

#[cfg(test)]
#[path = "identity_reuse/tests.rs"]
mod identity_reuse_tests;
