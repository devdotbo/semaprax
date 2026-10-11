//! Plan-infallible scalar Vec owner operations. Every cleanup-plan constructor
//! (`deferred_commit::is_infallible_vec_operation`) declares no status lane for
//! Clear and Sort, so no exact ReturnFailure exit exists for them in any
//! context. Like Core Wasm, a non-success runtime token is a fail-stop
//! invariant, never a selected or forged failure route.
use super::super::super::{backend_error, CEmitter, COutput};
use crate::diagnostic::Diagnostic;
use crate::hir::ResolvedExpr;

impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn emit_plan_infallible_vec_status(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::vec_ops::VecOp,
    ) -> Result<(), Diagnostic> {
        if !matches!(
            op,
            crate::vec_ops::VecOp::Clear | crate::vec_ops::VecOp::Sort
        ) {
            return Err(backend_error("native Vec operation is not plan-infallible"));
        }
        // The authenticated plan remains the authority: a declared status for
        // this occurrence means plan and backend disagree, so refuse.
        if self
            .function
            .cleanup_plan
            .status_sources
            .iter()
            .any(|source| source.id.expression == expr.id)
        {
            return Err(backend_error(
                "native plan-infallible Vec operation has a declared status source",
            ));
        }
        self.line(
            "if (spx_status != SPX_STATUS_SUCCESS) spx_runtime_invariant_failure(\"plan-infallible Vec operation returned failure\");",
        );
        Ok(())
    }
}
