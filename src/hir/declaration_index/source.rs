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

    /// The resolver's own pass index was built from the same `ProgramSource`
    /// and inserted every top-level type, interface, import, resource
    /// lifecycle, function, and class method before `Resolver::resolve`
    /// begins. Lookup is exact-text and borrowed; it shares the existing
    /// carrier and validates nothing (kind, owner, and profile checks stay at
    /// their callers). There is no legacy-construction fallback: every actual
    /// caller passes text read from that same source, and a detached index is
    /// outside this accessor's premise.
    pub(in crate::hir) fn held_identity(&self, text: &str) -> DeclarationId {
        #[cfg(test)]
        if let Some(saved) = identity_reuse_tests::resolver_held::saved_identity(text) {
            return saved;
        }
        self.declarations
            .get(text)
            .expect("resolver subject held by its own declaration index")
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
