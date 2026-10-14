use super::{hir_error, Diagnostic, HirValidator, ResolvedType};
impl HirValidator<'_> {
    pub(super) fn require_type(
        &self,
        actual: &ResolvedType,
        expected: &ResolvedType,
        context: &str,
    ) -> Result<(), Diagnostic> {
        if actual == expected {
            Ok(())
        } else {
            Err(hir_error(format!(
                "{context} has inconsistent resolved types"
            )))
        }
    }
}
