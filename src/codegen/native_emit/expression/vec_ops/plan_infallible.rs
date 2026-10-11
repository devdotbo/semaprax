//! Plan-infallible Vec operations across admitted element carriers. Every
//! cleanup-plan constructor
//! (`deferred_commit::is_infallible_vec_operation`) declares no status lane for
//! Len, Capacity, Clear, Sort, SortOwned and SortBy, so no exact failure exit
//! exists for them in any context. Like Core Wasm, a non-success runtime
//! token is a fail-stop invariant, never a selected or forged failure route.
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
            crate::vec_ops::VecOp::Clear
                | crate::vec_ops::VecOp::Sort
                | crate::vec_ops::VecOp::SortOwned
                | crate::vec_ops::VecOp::SortBy
        ) {
            return Err(backend_error("native Vec operation is not plan-infallible"));
        }
        self.validate_plan_infallible_vec_operation(expr, op)?;
        self.line(
            "if (spx_status != SPX_STATUS_SUCCESS) spx_runtime_invariant_failure(\"plan-infallible Vec operation returned failure\");",
        );
        Ok(())
    }

    pub(super) fn emit_vec_operation_status(
        &mut self,
        expr: &ResolvedExpr,
        op: crate::vec_ops::VecOp,
    ) -> Result<(), Diagnostic> {
        if matches!(
            op,
            crate::vec_ops::VecOp::Clear
                | crate::vec_ops::VecOp::Sort
                | crate::vec_ops::VecOp::SortOwned
                | crate::vec_ops::VecOp::SortBy
        ) {
            self.emit_plan_infallible_vec_status(expr, op)
        } else {
            self.emit_operation_failure_check()
        }
    }

    /// Value-returning Len/Capacity do not publish or inspect spx_status.
    /// Validate their absent lane without reading a token left by an earlier call.
    pub(super) fn validate_plan_infallible_vec_operation(
        &self,
        expr: &ResolvedExpr,
        op: crate::vec_ops::VecOp,
    ) -> Result<(), Diagnostic> {
        if !matches!(
            op,
            crate::vec_ops::VecOp::Len
                | crate::vec_ops::VecOp::Capacity
                | crate::vec_ops::VecOp::Clear
                | crate::vec_ops::VecOp::Sort
                | crate::vec_ops::VecOp::SortOwned
                | crate::vec_ops::VecOp::SortBy
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
        Ok(())
    }
}
