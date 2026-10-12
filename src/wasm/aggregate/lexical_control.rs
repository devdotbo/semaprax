//! Selective structured exits use exact certificate finalizers and label bindings.
use super::*;

impl Emitter<'_> {
    pub(super) fn emit_if(
        &mut self,
        expr: &ResolvedExpr,
        condition: &ResolvedExpr,
        then_branch: &ResolvedExpr,
        else_branch: &ResolvedExpr,
    ) -> Result<Value, Diagnostic> {
        let condition = self.emit_expr(condition)?;
        self.require_scalar(&condition, &ResolvedType::Bool, "if condition")?;
        let destination = if expr.ty == ResolvedType::Never {
            Value::Bottom
        } else if is_aggregate(self.program, &expr.ty)? {
            Value::Aggregate {
                pointer: self.plan.expr_pointer(expr)?,
                ty: expr.ty.clone(),
            }
        } else {
            Value::Scalar {
                local: self.plan.expr_scalar(expr)?,
                ty: expr.ty.clone(),
            }
        };
        self.get_scalar(&condition);
        self.output.extend([0x04, 0x40]);
        self.control_depth += 1;
        let then_value = self.emit_expr(then_branch)?;
        self.copy_value(&destination, &then_value, "then branch")?;
        self.output.push(0x05);
        let else_value = self.emit_expr(else_branch)?;
        self.copy_value(&destination, &else_value, "else branch")?;
        self.control_depth -= 1;
        self.output.push(0x0b);
        Ok(destination)
    }
}

pub(super) struct LoopLabel<'a> {
    certificate: &'a crate::cleanup_plan::LoopCertificate,
    break_depth: u32,
    continue_depth: u32,
}

impl LoopLabel<'_> {
    pub(super) fn latch(&self) -> &crate::hir::LoopLatchKind {
        &self.certificate.latch
    }
}

impl<'a> Emitter<'a> {
    pub(super) fn bind_lexical_loop(&mut self, condition: &ResolvedExpr) -> Result<(), Diagnostic> {
        let certificate = self
            .cleanup_plan
            .lexical_loop(&condition.id)
            .ok_or_else(|| error("lexical loop has no exact certificate"))?;
        if self.lexical_loops.len() == crate::cache_codec::MAX_DEPTH {
            return Err(error("lexical label depth exceeded"));
        }
        if self.lexical_loops.len() == self.lexical_loops.capacity() {
            let capacity = self
                .lexical_loops
                .capacity()
                .max(1)
                .checked_mul(2)
                .ok_or_else(|| error("lexical label capacity overflow"))?
                .min(crate::cache_codec::MAX_DEPTH);
            if !crate::bounded_output::reserve_active_required(
                capacity * std::mem::size_of::<LoopLabel<'_>>(),
            ) {
                return Err(error("lexical label allocation exceeds reservation"));
            }
            let mut next = Vec::with_capacity(capacity);
            next.append(&mut self.lexical_loops);
            self.lexical_loops = next;
        }
        self.lexical_loops.push(LoopLabel {
            certificate,
            break_depth: self.control_depth - 1,
            continue_depth: self.control_depth,
        });
        Ok(())
    }

    pub(super) fn lexical_result_stage(&self) -> Result<Value, Diagnostic> {
        if let Some(local) = self.plan.result_stage_scalar {
            Ok(Value::Scalar {
                local,
                ty: self.return_type.clone(),
            })
        } else {
            Ok(Value::Aggregate {
                pointer: Pointer {
                    local: self.plan.frame_base,
                    offset: self
                        .plan
                        .result_stage_aggregate
                        .ok_or_else(|| error("lexical result staging carrier is absent"))?,
                },
                ty: self.return_type.clone(),
            })
        }
    }

    pub(super) fn emit_lexical_exit(
        &mut self,
        expression: &ResolvedExpr,
        kind: crate::hir::LexicalExitKind,
        target: &crate::hir::ControlTarget,
        operand: Option<&ResolvedExpr>,
    ) -> Result<Value, Diagnostic> {
        use crate::cleanup_plan::ExitContinuation;
        use crate::hir::LexicalExitKind;
        let plan = self.cleanup_plan;
        let (certificate, route) = plan
            .lexical_exit(&expression.id)
            .ok_or_else(|| error("lexical exit lacks its exact certificate"))?;
        if certificate.kind != kind
            || &certificate.target != target
            || expression.ty != ResolvedType::Never
        {
            return Err(error("lexical exit certificate and expression disagree"));
        }
        let label = match (kind, &route.continuation, operand) {
            (
                LexicalExitKind::Return,
                ExitContinuation::FunctionReturn {
                    target: checked, ..
                },
                Some(operand),
            ) if checked == target => {
                let value = self.emit_expr(operand)?;
                let staged = self.lexical_result_stage()?;
                self.copy_value(&staged, &value, "lexical provisional result")?;
                self.apply_post_transitions(&expression.id, &staged)?;
                self.emit_cleanup_actions(&route.finalize_in_order)?;
                if let Some(local) = self.plan.lexical_return_resume {
                    let ordinal = match certificate.resume {
                        None => 0,
                        Some(entry) => plan
                            .lexical_control
                            .as_deref()
                            .unwrap()
                            .resumes
                            .iter()
                            .position(|resume| resume.entry == entry)
                            .and_then(|index| u32::try_from(index).ok())
                            .and_then(|index| index.checked_add(1))
                            .ok_or_else(|| error("return resume lacks its canonical ordinal"))?,
                    };
                    self.output.push(0x41);
                    write_i64(self.output, i64::from(ordinal));
                    self.output.push(0x21);
                    write_u32(self.output, local);
                } else if certificate.resume.is_some() {
                    return Err(error("return resume discriminator is absent"));
                }
                let ready = self
                    .plan
                    .result_staged
                    .ok_or_else(|| error("lexical return lacks staged-result guard"))?;
                self.output.extend([0x41, 0x01, 0x21]);
                write_u32(self.output, ready);
                0
            }
            (
                LexicalExitKind::Break,
                ExitContinuation::LoopBreak {
                    target: checked, ..
                },
                None,
            ) if checked == target => {
                let label = self
                    .lexical_loops
                    .last()
                    .filter(|label| &label.certificate.target == target)
                    .ok_or_else(|| error("break target is not the nearest bound loop"))?
                    .break_depth;
                self.emit_cleanup_actions(&route.finalize_in_order)?;
                label
            }
            (LexicalExitKind::Continue, ExitContinuation::LoopLatch { target: checked }, None)
                if checked == target =>
            {
                let loop_ = self
                    .lexical_loops
                    .last()
                    .filter(|label| &label.certificate.target == target)
                    .ok_or_else(|| error("continue target is not the nearest bound loop"))?;
                let certificate_loop = loop_.certificate;
                let label = loop_.continue_depth;
                self.emit_cleanup_actions(&route.finalize_in_order)?;
                let previous = self.lexical_resume;
                self.lexical_resume = certificate.resume;
                let outcome = self.emit_lexical_latch(&certificate_loop.latch);
                self.lexical_resume = previous;
                outcome?;
                label
            }
            _ => return Err(error("lexical exit has a mismatched typed continuation")),
        };
        self.output.push(0x0c);
        write_u32(
            self.output,
            self.control_depth
                .checked_sub(label)
                .ok_or_else(|| error("lexical branch is outside its bound depth"))?,
        );
        Ok(Value::Bottom)
    }

    fn emit_lexical_latch(&mut self, latch: &crate::hir::LoopLatchKind) -> Result<(), Diagnostic> {
        use crate::hir::LoopLatchKind;
        let function = self.function;
        match latch {
            LoopLatchKind::Condition => Ok(()),
            LoopLatchKind::SourceForStep { kind, step } => {
                if !matches!(
                    kind,
                    crate::hir::SourceForKind::Copy
                        | crate::hir::SourceForKind::BorrowRecord
                        | crate::hir::SourceForKind::BorrowBytes
                ) {
                    return Err(error("owned iterator needs its distinct typed latch"));
                }
                let (binding, value) =
                    crate::wasm::lexical_lookup::assignment(&function.body, step)?;
                let produced = self.emit_expr(value)?;
                let destination = self
                    .bindings
                    .get(&binding.id)
                    .cloned()
                    .ok_or_else(|| error("source For latch binding is absent"))?;
                self.copy_value(&destination, &produced, "source For latch")
            }
            LoopLatchKind::OwnedIteratorStep {
                next,
                assignment,
                step,
                remainder,
            } => {
                let (binding, value) =
                    crate::wasm::lexical_lookup::assignment(&function.body, assignment)?;
                if &binding.id != step {
                    return Err(error("owned latch Step binding differs"));
                }
                let ResolvedExprKind::Match { arms, .. } = &value.kind else {
                    return Err(error("owned latch assignment is not a Step match"));
                };
                let [_, yielded] = arms.as_slice() else {
                    return Err(error("owned latch case inventory differs"));
                };
                let crate::hir::ResolvedMatchPattern::Variant { fields, .. } = &yielded.pattern
                else {
                    return Err(error("owned latch Yield pattern differs"));
                };
                let [_, rest] = fields.as_slice() else {
                    return Err(error("owned latch Yield field inventory differs"));
                };
                let ResolvedExprKind::Block { tail, .. } = &yielded.value.kind else {
                    return Err(error("owned latch Yield contour differs"));
                };
                if &tail.id != next || &rest.binding.id != remainder {
                    return Err(error("owned latch next or remainder identity differs"));
                }
                let previous_epoch = self.take_latch_epoch(tail, remainder)?;
                let outcome = (|| {
                    let produced = self.emit_expr(tail)?;
                    // Complete only the authenticated Yield-arm tail and enclosing
                    // result handoffs. The authored arm prefix has already run.
                    let arm_result = self.materialize(&yielded.value, &produced)?;
                    let yield_storage =
                        crate::cleanup_plan::StorageId::Temporary(yielded.value.id.clone());
                    let phase = self.lexical_transfer_block(&yielded.value.id, &yield_storage)?;
                    self.apply_post_transitions_storage(
                        &yielded.value.id,
                        &arm_result,
                        &yield_storage,
                        true,
                    )?;
                    self.emit_lexical_transfer_scope(phase)?;
                    let joined = self.materialize(value, &arm_result)?;
                    let match_storage =
                        crate::cleanup_plan::StorageId::Temporary(assignment.clone());
                    let phase = self.lexical_transfer_block(&yielded.value.id, &match_storage)?;
                    self.apply_post_transitions_storage(
                        &yielded.value.id,
                        &joined,
                        &match_storage,
                        true,
                    )?;
                    self.emit_lexical_transfer_scope(phase)?;
                    let step_storage = crate::cleanup_plan::StorageId::Value(step.clone());
                    let phase = self.lexical_transfer_block(assignment, &step_storage)?;
                    self.apply_post_transitions_storage(assignment, &joined, &step_storage, true)?;
                    let destination = self
                        .bindings
                        .get(step)
                        .cloned()
                        .ok_or_else(|| error("owned latch Step carrier is absent"))?;
                    self.copy_value(&destination, &joined, "owned iterator latch")?;
                    self.emit_lexical_transfer_scope(phase)
                })();
                let restored = self.restore_latch_epoch(tail, previous_epoch);
                outcome?;
                restored
            }
        }
    }

    pub(super) fn emit_lexical_epilogues(&mut self) -> Result<(), Diagnostic> {
        let function = self.function;
        let plan = self.cleanup_plan;
        let Some(local) = self.plan.lexical_return_resume else {
            return self.emit_epilogue_in_context(None);
        };
        // Canonical resume order determines the private discriminator; do not
        // sort, merge or reinterpret either the domains or cleanup vectors.
        if plan.exits.iter().any(|exit| {
            plan.lexical_block_in_context(None, exit.from)
                && matches!(
                    exit.continuation,
                    crate::cleanup_plan::ExitContinuation::CommitResult { .. }
                )
        }) {
            self.emit_epilogue_case(local, 0, None)?;
        } else if function.body.ty != ResolvedType::Never
            || plan
                .lexical_control
                .as_deref()
                .unwrap()
                .exits
                .iter()
                .any(|exit| {
                    exit.kind == crate::hir::LexicalExitKind::Return && exit.resume.is_none()
                })
        {
            return Err(error(
                "reachable ordinary result has no exact success proof domain",
            ));
        }
        let control = plan
            .lexical_control
            .as_deref()
            .ok_or_else(|| error("resume local has no lexical proof"))?;
        for (index, resume) in control.resumes.iter().enumerate() {
            if !control.exits.iter().any(|exit| {
                exit.kind == crate::hir::LexicalExitKind::Return
                    && exit.resume == Some(resume.entry)
            }) {
                continue;
            }
            let ordinal = u32::try_from(index)
                .ok()
                .and_then(|index| index.checked_add(1))
                .ok_or_else(|| error("resume ordinal overflow"))?;
            self.emit_epilogue_case(local, ordinal, Some(resume.entry))?;
        }
        let _ = function;
        Ok(())
    }
    fn emit_epilogue_case(
        &mut self,
        local: u32,
        ordinal: u32,
        resume: Option<crate::cleanup_plan::BlockId>,
    ) -> Result<(), Diagnostic> {
        self.output.push(0x20);
        write_u32(self.output, local);
        self.output.push(0x41);
        write_i64(self.output, i64::from(ordinal));
        self.output.extend([0x46, 0x04, 0x40]);
        self.control_depth += 1;
        self.emit_epilogue_in_context(resume)?;
        self.control_depth -= 1;
        self.output.push(0x0b);
        Ok(())
    }
    fn emit_epilogue_in_context(
        &mut self,
        resume: Option<crate::cleanup_plan::BlockId>,
    ) -> Result<(), Diagnostic> {
        let previous = self.lexical_resume;
        self.lexical_resume = resume;
        let outcome = self.emit_selected_epilogue();
        self.lexical_resume = previous;
        outcome
    }
    fn emit_selected_epilogue(&mut self) -> Result<(), Diagnostic> {
        let function = self.function;
        for contract in &function.ensures {
            let condition = self.emit_expr(contract)?;
            self.require_scalar(&condition, &ResolvedType::Bool, "postcondition")?;
            self.get_scalar(&condition);
            self.output.push(0x45);
            self.failure_expression = Some(contract.id.clone());
            self.fail_if_in_lane(STATUS_ENSURES_FALSE, StatusLane::ContractFalse)?;
            self.failure_expression = None;
            self.emit_contract_scope_success(&contract.id)?;
        }
        self.emit_success_cleanup(&function.cleanup_plan)
    }
}

impl Emitter<'_> {
    pub(super) fn stage_function_result(&mut self, result: &Value) -> Result<Value, Diagnostic> {
        let plan = self.plan;
        let function = self.function;
        let owned_string_profile = self.owned_utf8_literals.is_some();
        let staged = if matches!(result, Value::Bottom) {
            self.lexical_result_stage()?
        } else if let Some(local) = plan.result_stage_scalar {
            self.require_scalar(result, &function.return_type, "function result")?;
            self.get_scalar(result);
            self.output.push(0x21);
            write_u32(self.output, local);
            if owned_string_profile && function.return_type == ResolvedType::String
                || crate::map_ops::is_collection(&function.return_type)
            {
                self.clear_scalar(result)?;
            }
            Value::Scalar {
                local,
                ty: function.return_type.clone(),
            }
        } else {
            let offset = plan
                .result_stage_aggregate
                .ok_or_else(|| error("missing aggregate result staging slot"))?;
            let staged = Value::Aggregate {
                pointer: Pointer {
                    local: plan.frame_base,
                    offset,
                },
                ty: function.return_type.clone(),
            };
            self.copy_value(&staged, result, "function result")?;
            staged
        };
        Ok(staged)
    }
}

impl Emitter<'_> {
    pub(super) fn lexical_region_in_context(
        &self,
        region: crate::cleanup_plan::CleanupRegionId,
    ) -> bool {
        self.cleanup_plan.lexical_control.is_none()
            || self.cleanup_plan.blocks.iter().any(|block| {
                block.region == region
                    && self
                        .cleanup_plan
                        .lexical_block_in_context(self.lexical_resume, block.id)
            })
    }
}

impl<'a> Emitter<'a> {
    pub(super) fn lexical_scope_end(
        &self,
        region: &crate::cleanup_plan::CleanupRegion,
    ) -> Option<&'a crate::cleanup_plan::ExitTarget> {
        if self.cleanup_plan.lexical_control.is_none() {
            return self
                .cleanup_plan
                .exits
                .get(region.normal_scope_end.0 as usize)
                .filter(|exit| exit.id == region.normal_scope_end);
        }
        let mut routes = self.cleanup_plan.exits.iter().filter(|exit| {
            self.cleanup_plan
                .lexical_block_in_context(self.lexical_resume, exit.from)
                && exit.leaves_regions.as_slice() == [region.id]
                && matches!(
                    exit.continuation,
                    crate::cleanup_plan::ExitContinuation::Continue(_)
                )
        });
        let route = routes.next()?;
        routes.next().is_none().then_some(route)
    }
}

impl Emitter<'_> {
    fn lexical_transfer_block(
        &self,
        at: &ExpressionId,
        destination: &crate::cleanup_plan::StorageId,
    ) -> Result<crate::cleanup_plan::BlockId, Diagnostic> {
        let mut blocks = self.cleanup_plan.blocks.iter().filter(|block| {
            self.cleanup_plan.lexical_block_in_context(self.lexical_resume, block.id)
                && block.transitions.iter().any(|transition| matches!(transition,
                    crate::cleanup_plan::CleanupTransition::Transfer { at: anchor, destination: place, .. }
                    | crate::cleanup_plan::CleanupTransition::TransferVariant { at: anchor, destination: place, .. }
                    | crate::cleanup_plan::CleanupTransition::Renew { at: anchor, destination: place, .. }
                    if anchor == at && &place.storage == destination))
        });
        let block = blocks
            .next()
            .ok_or_else(|| error("lexical transfer phase has no exact block"))?;
        if blocks.next().is_some() {
            return Err(error("lexical transfer phase repeats its exact block"));
        }
        Ok(block.id)
    }
    fn emit_lexical_transfer_scope(
        &mut self,
        block: crate::cleanup_plan::BlockId,
    ) -> Result<(), Diagnostic> {
        let region = self
            .cleanup_plan
            .blocks
            .get(block.0 as usize)
            .filter(|candidate| candidate.id == block)
            .and_then(|candidate| self.cleanup_plan.regions.get(candidate.region.0 as usize))
            .ok_or_else(|| error("lexical transfer scope block is absent"))?;
        let exit = self
            .lexical_scope_end(region)
            .filter(|exit| exit.from == block)
            .ok_or_else(|| error("lexical transfer scope is not its exact continuation"))?;
        self.emit_cleanup_actions(&exit.finalize_in_order)
    }
}

impl Emitter<'_> {
    pub(super) fn emit_execution_boundary_cleanup(
        &mut self,
        kind: crate::cleanup_plan::ExecutionBoundaryKind,
        site: &ExpressionId,
    ) -> Result<(), Diagnostic> {
        let boundary = self
            .cleanup_plan
            .execution_boundary(kind, site, self.lexical_resume)
            .ok_or_else(|| error("execution guard has no exact cleanup boundary"))?;
        self.emit_cleanup_actions(&boundary.finalize_in_order)
    }
}
