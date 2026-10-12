//! Caller-held exact-program canonical lookup metadata. No admission or payer.
use super::{Diagnostic, ExpressionId, Place, ResolvedProgram};
use crate::loan_plan::{LoanCause, LoanId, LoanPointPhase};
pub(super) mod caller;
#[cfg(test)]
mod tests;

type IdKey = (ExpressionId, LoanCause);
type LiveKey = (ExpressionId, LoanPointPhase, LoanId);

pub(super) struct Storage<'a> {
    program: &'a ResolvedProgram,
    ids: Vec<(IdKey, LoanId)>,
    liveness: Vec<(LiveKey, usize)>,
    origins: Vec<Place>,
}
impl Storage<'_> {
    pub(super) fn is_current(&self, program: &ResolvedProgram) -> bool {
        std::ptr::eq(self.program, program)
    }
    pub(super) fn id(&self, key: &IdKey) -> Option<&LoanId> {
        self.ids
            .binary_search_by(|entry| entry.0.cmp(key))
            .ok()
            .map(|i| &self.ids[i].1)
    }
    pub(super) fn live(&self, key: &LiveKey) -> Option<&usize> {
        self.liveness
            .binary_search_by(|entry| entry.0.cmp(key))
            .ok()
            .map(|i| &self.liveness[i].1)
    }
    pub(super) fn origin(&self, index: usize) -> Option<&Place> {
        self.origins.get(index)
    }
}
