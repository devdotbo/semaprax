//! Forecast attribution is owned by one workspace invocation, not its thread.
//! Move the prior diagnostic context aside; no ledger or receipt is modified.
use super::*;
pub(in crate::workspace_graph) struct Scope {
    phase: BuilderPhase,
    modules: usize,
    bytes: usize,
    dominant: Option<(String, usize)>,
    pending: Option<String>,
    sequential: Option<SequentialRefusal>,
}
pub(in crate::workspace_graph) fn enter() -> Scope {
    Scope {
        phase: BUILDER_PHASE.with(|value| value.replace(BuilderPhase::Live)),
        modules: PRECHARGE_MODULES.with(|value| value.replace(0)),
        bytes: PRECHARGE_BYTES.with(|value| value.replace(0)),
        dominant: PRECHARGE_DOMINANT.with(|value| value.replace(None)),
        pending: PRECHARGE_PENDING.with(|value| value.replace(None)),
        sequential: PRECHARGE_SEQUENTIAL.with(|value| value.replace(None)),
    }
}
impl Drop for Scope {
    fn drop(&mut self) {
        BUILDER_PHASE.with(|value| value.set(self.phase));
        PRECHARGE_MODULES.with(|value| value.set(self.modules));
        PRECHARGE_BYTES.with(|value| value.set(self.bytes));
        // The returned current context drops here after error rendering has
        // copied its exact help into the returned Diagnostic. Nested callers
        // regain their original allocation and counters without cloning them.
        PRECHARGE_DOMINANT.with(|value| drop(value.replace(self.dominant.take())));
        PRECHARGE_PENDING.with(|value| drop(value.replace(self.pending.take())));
        PRECHARGE_SEQUENTIAL.with(|value| drop(value.replace(self.sequential.take())));
    }
}
#[cfg(test)]
mod tests;
