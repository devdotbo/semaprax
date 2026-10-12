//! Sealed semantic error data. These atoms grant no replay/lookup authority.
//! Only the explicitly unfunded legacy adapter formats detailed H006 text.
use super::*;
#[derive(Debug)]
pub(super) enum Atom {
    RepeatedStatus(ExpressionId),
    UnknownStatus(ExpressionId),
    NoncontiguousSlot(usize),
    RepeatedStorage(StorageId),
    DeadCallFlag(ExpressionId, LivenessFlagId),
    RepeatedCallFlag(ExpressionId, LivenessFlagId),
    RepeatedVariantEpoch(ExpressionId),
    MismatchedFailure(ExpressionId),
}
#[derive(Debug)]
pub(crate) struct Receipt {
    #[cfg_attr(
        not(test),
        allow(
            dead_code,
            reason = "shared source anchor retained with the semantic receipt"
        )
    )]
    source: ExpressionId,
    atom: Atom,
}
impl Receipt {
    pub(super) fn legacy(self, function: &ResolvedFunction) -> Diagnostic {
        let message = match self.atom {
            Atom::RepeatedStatus(id) => format!("status source for `{id}` is repeated"),
            Atom::UnknownStatus(id) => {
                format!("status source references unknown expression `{id}`")
            }
            Atom::NoncontiguousSlot(index) => {
                format!("cleanup slot at index {index} has non-contiguous identity or index")
            }
            Atom::DeadCallFlag(call, flag) => format!(
                "call `{call}` atomically consumes dead argument flag {}",
                flag.0
            ),
            Atom::RepeatedCallFlag(call, flag) => format!(
                "call `{call}` atomically consumes argument flag {} twice",
                flag.0
            ),
            Atom::RepeatedVariantEpoch(call) => {
                format!("call `{call}` atomically consumes a variant epoch twice")
            }
            Atom::MismatchedFailure(expression) => {
                format!("selected failure `{expression}` does not match the pending failing edge")
            }
            Atom::RepeatedStorage(storage) => format!("cleanup storage `{storage:?}` is repeated"),
        };
        replay_error(function, message)
    }
    #[cfg(test)]
    pub(super) fn source(&self) -> &ExpressionId {
        &self.source
    }
}
pub(super) trait Error: From<Diagnostic> {
    fn structural(function: &ResolvedFunction, atom: Atom) -> Self;
}
impl Error for Diagnostic {
    fn structural(function: &ResolvedFunction, atom: Atom) -> Self {
        Receipt {
            source: function.body.id.clone(),
            atom,
        }
        .legacy(function)
    }
}
impl Error for expression_facts::errors::EngineError {
    fn structural(function: &ResolvedFunction, atom: Atom) -> Self {
        Self::Structural(Receipt {
            source: function.body.id.clone(),
            atom,
        })
    }
}
#[cfg(test)]
mod tests;
