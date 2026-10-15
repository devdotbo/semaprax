//! Complete original compatibility constructors, independently compiled.
use super::*;
impl<'a> HirValidator<'a> {
    #[cfg(test)]
    pub(super) fn original_with_directory(
        program: &'a ResolvedProgram,
        directory: Option<&'a super::function_directory::Directory<'a>>,
    ) -> Result<Self, Diagnostic> {
        let seed = constructor_seed::ConstructorSeed::authenticate(program, directory)?;
        let canonical_loans = canonical_loans::Inventory::rebuild(program)?;
        Self::from_constructor(seed, canonical_loans::Lookup::Legacy(canonical_loans), None)
    }
    pub(super) fn original_with_profiles(
        program: &'a ResolvedProgram,
        directory: Option<&'a super::function_directory::Directory<'a>>,
        storage: Option<&'a super::canonical_loan_storage::Storage<'a>>,
        profiles: executable_profiles::Lookup<'a>,
    ) -> Result<Self, Diagnostic> {
        let seed = constructor_seed::ConstructorSeed::authenticate(program, directory)?;
        let canonical_loans = if let Some(storage) = storage {
            if !storage.is_current(seed.program) {
                return Err(hir_error(
                    "canonical shared-loan storage does not belong to the current program",
                ));
            }
            canonical_loans::Lookup::Borrowed(storage)
        } else {
            canonical_loans::Lookup::Legacy(canonical_loans::Inventory::rebuild(program)?)
        };
        Self::from_constructor(seed, canonical_loans, Some(profiles))
    }
}
