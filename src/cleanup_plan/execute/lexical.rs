//! Loop-latch continuation for the conformance executor (cleanup plan v18).
use super::*;
use crate::cleanup_plan::ExitTargetId;

impl Executor<'_> {
    /// Where a loop latch continues. A certified `continue` first runs its
    /// finite generated advancement fragment. Otherwise the iteration is the one
    /// admitted linearized iteration, exactly like a legacy `while`, after which
    /// the loop ends at its `after` block. The proof CFG never has a back edge.
    pub(super) fn latch_successor(
        &self,
        exit: ExitTargetId,
    ) -> Result<BlockId, CleanupExecutionError> {
        let plan = &self.function.cleanup_plan;
        let control = plan
            .lexical_control
            .as_deref()
            .ok_or_else(|| invariant("loop latch has no lexical-control certificate"))?;
        if let Some(entry) = control
            .exits
            .iter()
            .find(|certificate| certificate.exit == exit)
            .and_then(|certificate| certificate.resume)
        {
            return Ok(entry);
        }
        let latch = plan
            .exits
            .iter()
            .find(|candidate| candidate.id == exit)
            .ok_or_else(|| invariant("loop latch exit is missing"))?;
        let ExitContinuation::LoopLatch {
            target: crate::hir::ControlTarget::Loop { site, .. },
        } = &latch.continuation
        else {
            return Err(invariant("loop latch exit has no loop target"));
        };
        control
            .loops
            .iter()
            .find(|certificate| &certificate.condition == site)
            .map(|certificate| certificate.after)
            .ok_or_else(|| invariant("loop latch names an uncertified loop"))
    }
}
