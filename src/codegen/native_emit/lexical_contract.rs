//! Successful contract scopes retain the selected lexical proof domain.
use super::*;
use crate::cleanup_plan::{ExitContinuation, StatusLane, StatusSourceId};

impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn emit_lexical_contract_success(
        &mut self,
        contract: &ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        let plan = &self.function.cleanup_plan;
        if plan.lexical_control.is_none() {
            return Ok(());
        }
        let source = StatusSourceId {
            expression: contract.id.clone(),
            lane: StatusLane::ContractFalse,
        };
        let mut failures = plan.exits.iter().filter(|exit| {
            matches!(&exit.continuation, ExitContinuation::ReturnFailure { source: candidate } if candidate == &source)
                && plan.lexical_block_in_context(self.cleanup_resume, exit.from)
        });
        let failure = failures.next().ok_or_else(|| {
            backend_error("native contract success lacks selected failure contour")
        })?;
        if failures.next().is_some() {
            return Err(backend_error(
                "native contract success has ambiguous failure contour",
            ));
        }
        let block = plan
            .blocks
            .get(failure.from.0 as usize)
            .filter(|block| block.id == failure.from)
            .ok_or_else(|| backend_error("native contract contour block is absent"))?;
        let exit = plan
            .lexical_scope_end(block.region, self.cleanup_resume)
            .ok_or_else(|| backend_error("native contract success lacks exact scope completion"))?;
        let cleanup = match self.bytes_plan {
            Some(view) => view.exit_finalizers(exit, false)?,
            None if exit.finalize_in_order.is_empty() => String::new(),
            None => {
                return Err(backend_error(
                    "native contract success lacks physical cleanup slots",
                ))
            }
        };
        for line in cleanup.lines() {
            self.line(line);
        }
        Ok(())
    }
}
