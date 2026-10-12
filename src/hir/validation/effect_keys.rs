//! Function-local borrowed effect keys; membership only, never a capability.
use super::*;
#[derive(Clone, Copy)]
pub(in crate::hir) enum Allowed<'a> {
    Borrowed(&'a BTreeSet<&'a str>),
    #[cfg(test)]
    Owned(&'a BTreeSet<String>),
}
impl Allowed<'_> {
    pub(super) fn contains(self, effect: &str) -> bool {
        match self {
            Self::Borrowed(keys) => keys.contains(effect),
            #[cfg(test)]
            Self::Owned(keys) => keys.contains(effect),
        }
    }
}

// Existing owned-set callers and the saved recursive oracle retain their entry.
#[cfg(test)]
impl HirValidator<'_> {
    #[allow(clippy::too_many_arguments)]
    pub(in crate::hir) fn validate_expr_iterative(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
        path: &str,
        allow_moves: bool,
        allowed_effects: Option<&BTreeSet<String>>,
    ) -> Result<(), Diagnostic> {
        self.validate_expr_iterative_effects(
            function,
            expression,
            scope,
            path,
            allow_moves,
            allowed_effects.map(Allowed::Owned),
        )
    }
    #[allow(clippy::too_many_arguments)]
    pub(in crate::hir) fn validate_expr_iterative_owned_oracle(
        &mut self,
        function: &FunctionExecutionId,
        expression: &ResolvedExpr,
        scope: &mut BTreeMap<ValueId, ValidationBinding>,
        path: &str,
        allow_moves: bool,
        allowed_effects: Option<&BTreeSet<String>>,
    ) -> Result<(), Diagnostic> {
        self.validate_expr_iterative_owned_reference(
            function,
            expression,
            scope,
            path,
            allow_moves,
            allowed_effects.map(Allowed::Owned),
        )
    }
}

#[cfg(test)]
mod tests;
