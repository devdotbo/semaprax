//! Exact context-selected terminal cleanup before the common ABI publication.
use super::*;
use crate::cleanup_plan::{ExitContinuation, ExitTarget, StatusLane, StatusSourceId};
impl<'a, O: COutput> CEmitter<'a, O> {
    fn canonical_terminal_code(&self, exit: &ExitTarget) -> Result<String, Diagnostic> {
        let mut cleanup = match self.bytes_plan {
            Some(plan) => plan.exit_finalizers(exit, true)?,
            None if exit.finalize_in_order.is_empty() => String::new(),
            None => {
                return Err(backend_error(
                    "native terminal route lacks physical cleanup slots",
                ))
            }
        };
        cleanup.push_str("spx_canonical_cleanup_done = true;\ngoto spx_epilogue;\n");
        Ok(cleanup)
    }
    fn emit_canonical_terminal(&mut self, exit: &ExitTarget) -> Result<(), Diagnostic> {
        for line in self.canonical_terminal_code(exit)?.lines() {
            self.line(line);
        }
        Ok(())
    }
    pub(super) fn emit_failure_at(
        &mut self,
        site: &ExpressionId,
        lane: StatusLane,
    ) -> Result<(), Diagnostic> {
        for line in self.failure_jump_at(site, lane)?.lines() {
            self.line(line);
        }
        Ok(())
    }
    pub(super) fn failure_jump_at(
        &self,
        site: &ExpressionId,
        lane: StatusLane,
    ) -> Result<String, Diagnostic> {
        if self.function.cleanup_plan.lexical_control.is_none() {
            return Ok("goto spx_epilogue;".to_owned());
        }
        let function = self.function;
        let plan = &function.cleanup_plan;
        let source = StatusSourceId {
            expression: site.clone(),
            lane,
        };
        let mut matches = plan.exits.iter().filter(|exit| {
            matches!(&exit.continuation, ExitContinuation::ReturnFailure { source: selected } if selected == &source)
                && plan.lexical_block_in_context(self.cleanup_resume, exit.from)
        });
        let exit = matches
            .next()
            .ok_or_else(|| backend_error("native failure has no exact context-selected exit"))?;
        if matches.next().is_some() {
            return Err(backend_error(
                "native failure exit is ambiguous in selected context",
            ));
        }
        self.canonical_terminal_code(exit)
    }
    pub(super) fn emit_operation_failure_check(&mut self) -> Result<(), Diagnostic> {
        if self.function.cleanup_plan.lexical_control.is_none() {
            self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
            return Ok(());
        }
        let site = self
            .active_expression
            .clone()
            .ok_or_else(|| backend_error("native operation failure lacks expression anchor"))?;
        self.emit_operation_failure_check_at(&site)
    }
    pub(super) fn emit_operation_failure_check_at(
        &mut self,
        site: &ExpressionId,
    ) -> Result<(), Diagnostic> {
        if self.function.cleanup_plan.lexical_control.is_none() {
            self.line("if (spx_status != SPX_STATUS_SUCCESS) goto spx_epilogue;");
            return Ok(());
        }
        self.line("if (spx_status != SPX_STATUS_SUCCESS) {");
        self.indent += 1;
        self.emit_failure_at(site, StatusLane::OperationFailure)?;
        self.indent -= 1;
        self.line("}");
        Ok(())
    }
    pub(super) fn emit_success_terminal(&mut self) -> Result<(), Diagnostic> {
        let function = self.function;
        let plan = &function.cleanup_plan;
        let Some(control) = plan.lexical_control.as_deref() else {
            self.line("goto spx_epilogue;");
            return Ok(());
        };
        let mut matches = control
            .function_success_exits
            .iter()
            .filter_map(|id| plan.exits.get(id.0 as usize).filter(|exit| exit.id == *id))
            .filter(|exit| plan.lexical_block_in_context(self.cleanup_resume, exit.from));
        let exit = matches
            .next()
            .ok_or_else(|| backend_error("native success has no exact root commit exit"))?;
        if matches.next().is_some()
            || !matches!(
                exit.continuation,
                ExitContinuation::CommitResult { .. } | ExitContinuation::ReturnUnit
            )
        {
            return Err(backend_error(
                "native root success exit is ambiguous or not a commit",
            ));
        }
        self.emit_canonical_terminal(exit)
    }
    pub(super) fn emit_postconditions(&mut self) -> Result<(), Diagnostic> {
        let function = self.function;
        let labels = self.contract_labels;
        for contract in &function.ensures {
            let condition = self.emit_expr(contract)?;
            self.require_type(&condition.ty, &ResolvedType::Bool, "postcondition")?;
            self.line(&format!("if (!({})) {{", condition.code));
            self.indent += 1;
            emit_contract_status(
                self,
                function,
                "SPX_STATUS_CONTRACT_ENSURES_FALSE",
                "ensures",
                contract_label(contract, labels),
            );
            self.emit_failure_at(&contract.id, StatusLane::ContractFalse)?;
            self.indent -= 1;
            self.line("}");
        }
        self.emit_success_terminal()
    }
}

impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn operation_failure_jump(&self) -> Result<String, Diagnostic> {
        if self.function.cleanup_plan.lexical_control.is_none() {
            return Ok("goto spx_epilogue;".to_owned());
        }
        let site = self
            .active_expression
            .as_ref()
            .ok_or_else(|| backend_error("native operation failure lacks expression anchor"))?;
        self.failure_jump_at(site, StatusLane::OperationFailure)
    }
}

impl<O: COutput> CEmitter<'_, O> {
    pub(super) fn emit_execution_boundary_failure(
        &mut self,
        kind: crate::cleanup_plan::ExecutionBoundaryKind,
        site: &ExpressionId,
    ) -> Result<(), Diagnostic> {
        if self.function.cleanup_plan.lexical_control.is_none() {
            self.line("goto spx_epilogue;");
            return Ok(());
        }
        let boundary = self
            .function
            .cleanup_plan
            .execution_boundary(kind, site, self.cleanup_resume)
            .ok_or_else(|| backend_error("native execution guard has no exact cleanup boundary"))?;
        let cleanup = match self.bytes_plan {
            Some(plan) => plan.finalizers_in_order(&boundary.finalize_in_order, true)?,
            None if boundary.finalize_in_order.is_empty() => String::new(),
            None => {
                return Err(backend_error(
                    "native execution guard lacks physical cleanup slots",
                ))
            }
        };
        for line in cleanup.lines() {
            self.line(line);
        }
        self.line("spx_canonical_cleanup_done = true;");
        self.line("goto spx_epilogue;");
        Ok(())
    }
}
