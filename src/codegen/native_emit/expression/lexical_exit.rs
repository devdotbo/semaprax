//! Certificate-bound lexical routes; generated labels confer no authority.
use super::*;
use crate::ast::LexicalExitKind;
use crate::cleanup_plan::ExitContinuation;
use crate::hir::{ControlTarget, LoopLatchKind};

impl<'a, O: COutput> CEmitter<'a, O> {
    pub(in crate::codegen::native_emit) fn require_flow_type(
        &self,
        actual: &ResolvedType,
        expected: &ResolvedType,
        context: &str,
    ) -> Result<(), Diagnostic> {
        if *actual == ResolvedType::Never {
            Ok(())
        } else {
            self.require_type(actual, expected, context)
        }
    }
    pub(super) fn lexical_loop_index(&self, target: &ControlTarget) -> Result<usize, Diagnostic> {
        let plan = self
            .function
            .cleanup_plan
            .lexical_control
            .as_deref()
            .ok_or_else(|| backend_error("native loop target lacks lexical certificate"))?;
        let mut found = plan
            .loops
            .iter()
            .enumerate()
            .filter(|(_, item)| &item.target == target);
        let (index, _) = found
            .next()
            .ok_or_else(|| backend_error("native loop target is absent"))?;
        if found.next().is_some() {
            return Err(backend_error("native loop target is ambiguous"));
        }
        Ok(index)
    }

    fn lexical_loop_label(
        &self,
        target: &ControlTarget,
        latch: bool,
    ) -> Result<String, Diagnostic> {
        Ok(format!(
            "spx_lexical_{}_{}",
            if latch { "latch" } else { "break" },
            self.lexical_loop_index(target)?
        ))
    }

    pub(super) fn emit_lexical_step_label(
        &mut self,
        site: &ExpressionId,
    ) -> Result<(), Diagnostic> {
        // Generated Continue paths inline their exact certified advancement.
        // The normal tail is never a shared resume label.
        let _ = site;
        Ok(())
    }

    fn emit_lexical_advance(
        &mut self,
        target: &ControlTarget,
        resume: Option<crate::cleanup_plan::BlockId>,
    ) -> Result<(), Diagnostic> {
        let condition = match target {
            ControlTarget::Loop { site, .. } => site,
            _ => return Err(backend_error("native continue target is not a loop")),
        };
        let latch = self
            .function
            .cleanup_plan
            .lexical_loop(condition)
            .filter(|certificate| &certificate.target == target)
            .ok_or_else(|| backend_error("native continue loop certificate is absent"))?
            .latch
            .clone();
        match latch {
            LoopLatchKind::Condition => {
                if resume.is_some() {
                    return Err(backend_error(
                        "native condition continue has unexpected resume",
                    ));
                }
            }
            LoopLatchKind::SourceForStep { kind, step } => {
                if !matches!(
                    kind,
                    hir::SourceForKind::Copy | hir::SourceForKind::BorrowRecord
                ) {
                    return Err(backend_error("native source-for step kind is not scalar"));
                }
                let (binding, value) =
                    super::lexical_lookup::assignment(&self.function.body, &step)?;
                let entry = resume
                    .ok_or_else(|| backend_error("native source-for continue lacks resume"))?;
                self.in_cleanup_resume(entry, |emitter| {
                    let result = emitter.emit_expr(value)?;
                    emitter.require_type(&result.ty, &binding.ty, "source-for latch assignment")?;
                    let name = emitter
                        .variables
                        .get(&binding.id)
                        .ok_or_else(|| backend_error("native source-for latch binding is absent"))?
                        .name
                        .clone();
                    emitter.line(&format!("{name} = {};", result.code));
                    Ok(())
                })?;
            }
            LoopLatchKind::OwnedIteratorStep {
                next,
                assignment,
                step,
                remainder,
            } => {
                let (binding, join) =
                    super::lexical_lookup::assignment(&self.function.body, &assignment)?;
                let (arm, blocks, block_count) = super::lexical_lookup::iterator_tail(join, &next)?;
                let next = super::lexical_lookup::expression(&self.function.body, &next)?;
                if binding.id != step || !crate::iterator_ops::is_step(&binding.ty) {
                    return Err(backend_error("native iterator latch binding differs"));
                }
                // Independent HIR validation binds this argument to the Yield
                // remainder. The emitter also requires its live local carrier.
                if !self.variables.contains_key(&remainder) {
                    return Err(backend_error("native iterator latch remainder is absent"));
                }
                let entry =
                    resume.ok_or_else(|| backend_error("native iterator continue lacks resume"))?;
                self.in_cleanup_resume(entry, |emitter| {
                    let result = emitter.emit_expr(next)?;
                    emitter.require_type(&result.ty, &binding.ty, "iterator latch result")?;
                    let layout = emitter.variant_layout(&binding.ty)?;
                    for block in blocks[..block_count].iter().rev().flatten() {
                        let phase = emitter
                            .bytes_plan
                            .ok_or_else(|| {
                                backend_error("native iterator resume lacks producer slots")
                            })?
                            .apply_variant_into_at(
                                &block.id,
                                &result.code,
                                &layout,
                                &crate::cleanup_plan::StorageId::Temporary(block.id.clone()),
                            )?;
                        for line in phase.lines() {
                            emitter.line(line);
                        }
                        emitter.emit_block_plan_scope_exit(block)?;
                    }
                    let phase = emitter
                        .bytes_plan
                        .ok_or_else(|| backend_error("native iterator resume lacks join slots"))?
                        .apply_variant_into_at(
                            &arm.value.id,
                            &result.code,
                            &layout,
                            &crate::cleanup_plan::StorageId::Temporary(join.id.clone()),
                        )?;
                    for line in phase.lines() {
                        emitter.line(line);
                    }
                    let carrier = emitter.temporary(&binding.ty)?;
                    emitter.copy_variant_join_carrier(&carrier, &result.code, &binding.ty)?;
                    let hir::ResolvedMatchPattern::Variant { case, fields, .. } = &arm.pattern
                    else {
                        return Err(backend_error(
                            "native iterator tail arm lacks variant bindings",
                        ));
                    };
                    let anchors = fields
                        .iter()
                        .filter(|field| {
                            is_direct_plan_owned(emitter.program, &field.binding.ty)
                                || match &join.kind {
                                    ResolvedExprKind::Match { scrutinee, .. } => {
                                        super::record_iterator_item::is_exact(
                                            emitter.program,
                                            &scrutinee.ty,
                                            case,
                                            &field.field,
                                            &field.binding.ty,
                                        )
                                    }
                                    _ => false,
                                }
                        })
                        .map(|field| {
                            crate::cleanup_plan::StorageId::Value(field.binding.id.clone())
                        })
                        .collect();
                    let cleanup = emitter
                        .bytes_plan
                        .ok_or_else(|| backend_error("native iterator resume lacks slots"))?
                        .scope_exit(&anchors)?;
                    for line in cleanup.lines() {
                        emitter.line(line);
                    }
                    emitter.finish_variant_join(join, &carrier)?;
                    let name = emitter
                        .variables
                        .get(&step)
                        .ok_or_else(|| backend_error("native iterator latch step is absent"))?
                        .name
                        .clone();
                    emitter.copy_variant_join_carrier(&name, &carrier, &binding.ty)?;
                    let loop_body = emitter
                        .function
                        .cleanup_plan
                        .lexical_loop(condition)
                        .ok_or_else(|| backend_error("native iterator resume lost its loop"))?
                        .body
                        .clone();
                    let loop_body =
                        super::lexical_lookup::expression(&emitter.function.body, &loop_body)?;
                    emitter.emit_block_plan_scope_exit(loop_body)
                })?;
            }
        }
        let label = self.lexical_loop_label(target, true)?;
        self.line(&format!("goto {label};"));
        Ok(())
    }

    pub(super) fn emit_lexical_exit(
        &mut self,
        expression: &ResolvedExpr,
        kind: LexicalExitKind,
        target: &ControlTarget,
        value: Option<&ResolvedExpr>,
    ) -> Result<CValue, Diagnostic> {
        let (certificate, exit) = self
            .function
            .cleanup_plan
            .lexical_exit(&expression.id)
            .ok_or_else(|| backend_error("native lexical exit lacks exact certificate"))?;
        if certificate.kind != kind
            || &certificate.target != target
            || expression.ty != ResolvedType::Never
        {
            return Err(backend_error(
                "native lexical exit disagrees with certificate",
            ));
        }
        let continuation = exit.continuation.clone();
        let resume = certificate.resume;
        match (&continuation, kind, target) {
            (
                ExitContinuation::LoopLatch { target: expected },
                LexicalExitKind::Continue,
                ControlTarget::Loop { .. },
            )
            | (
                ExitContinuation::LoopBreak {
                    target: expected, ..
                },
                LexicalExitKind::Break,
                ControlTarget::Loop { .. },
            ) if expected == target && value.is_none() => {}
            (
                ExitContinuation::FunctionReturn {
                    target: expected, ..
                },
                LexicalExitKind::Return,
                ControlTarget::Function(owner),
            ) if expected == target && owner == self.execution && value.is_some() => {}
            _ => {
                return Err(backend_error(
                    "native lexical continuation is not the selected typed exit",
                ))
            }
        }
        if let Some(value) = value {
            let result = self.emit_expr(value)?;
            self.require_flow_type(&result.ty, self.return_type, "lexical return value")?;
            if result.ty == ResolvedType::Never {
                return Ok(result);
            }
            self.apply_owned_plan_at_value(&expression.id, &result)?;
            if result.ty == ResolvedType::String && self.owned_strings.is_some() {
                self.string_move("spx_result", &result.code);
            } else if !is_direct_plan_owned(self.program, &result.ty) {
                if self.record_contains_owned_bytes(&result.ty)? {
                    self.move_owned_record_fields("spx_result", &result.code, &result.ty)?;
                } else {
                    self.line(&format!("spx_result = {};", result.code));
                }
            }
            if super::super::expression_has_try(&self.function.body) {
                self.line("spx_result_staged = true;");
            }
        }
        let finalizers = match self.bytes_plan {
            Some(plan) => plan.lexical_finalizers(&self.function.cleanup_plan, &expression.id)?,
            None if exit.finalize_in_order.is_empty() => String::new(),
            None => {
                return Err(backend_error(
                    "native lexical exit lacks physical cleanup slots",
                ))
            }
        };
        for line in finalizers.lines() {
            self.line(line);
        }
        match kind {
            LexicalExitKind::Return => {
                let entry = resume
                    .ok_or_else(|| backend_error("native return lacks ensures resume domain"))?;
                self.in_cleanup_resume(entry, |emitter| emitter.emit_postconditions())?;
            }
            LexicalExitKind::Continue => self.emit_lexical_advance(target, resume)?,
            LexicalExitKind::Break => {
                if let Some(entry) = resume {
                    let ExitContinuation::LoopBreak { edge, .. } = continuation else {
                        return Err(backend_error("native owning Break lacks typed edge"));
                    };
                    if self
                        .function
                        .cleanup_plan
                        .edges
                        .get(edge.0 as usize)
                        .is_none_or(|edge| edge.to != entry)
                    {
                        return Err(backend_error(
                            "native owning Break edge differs from its resume",
                        ));
                    }
                    self.emit_owned_wrapper_break(target, entry)?;
                } else {
                    let label = self.lexical_loop_label(target, false)?;
                    self.line(&format!("goto {label};"));
                }
            }
        }
        Ok(CValue {
            code: String::new(),
            ty: ResolvedType::Never,
        })
    }

    pub(super) fn emit_while(
        &mut self,
        condition: &ResolvedExpr,
        body: &ResolvedExpr,
    ) -> Result<(), Diagnostic> {
        let target = ControlTarget::Loop {
            owner: self.execution.clone(),
            site: condition.id.clone(),
        };
        let certificate = self
            .function
            .cleanup_plan
            .lexical_control
            .as_deref()
            .and_then(|control| control.loops.iter().find(|item| item.target == target));
        if let Some(item) = certificate {
            if item.condition != condition.id || item.body != body.id {
                return Err(backend_error(
                    "native loop certificate has wrong body or condition",
                ));
            }
        }
        let references = |kind| {
            self.function
                .cleanup_plan
                .lexical_control
                .as_deref()
                .is_some_and(|control| {
                    control
                        .exits
                        .iter()
                        .any(|item| item.target == target && item.kind == kind)
                })
        };
        let has_continue = references(LexicalExitKind::Continue);
        let has_break = self
            .function
            .cleanup_plan
            .lexical_control
            .as_deref()
            .is_some_and(|control| {
                control.exits.iter().any(|item| {
                    item.target == target
                        && item.kind == LexicalExitKind::Break
                        && item.resume.is_none()
                })
            });
        self.line("for (;;) {");
        self.indent += 1;
        if has_continue {
            let label = self.lexical_loop_label(&target, true)?;
            self.label(&label);
        }
        let condition_value = self.emit_expr(condition)?;
        self.require_type(&condition_value.ty, &ResolvedType::Bool, "while condition")?;
        self.emit_scalar_match_guard_scope_exit(condition)?;
        self.line(&format!("if (!({})) break;", condition_value.code));
        self.semantic_charge(
            crate::cleanup_plan::ExecutionBoundaryKind::LoopIteration,
            &condition.id,
        )?;
        let body_value = self.emit_expr(body)?;
        if matches!(body_value.ty, ResolvedType::String) {
            return Err(backend_error(
                "discarding an owned string has no admitted native lowering",
            ));
        }
        if body_value.ty != ResolvedType::Never {
            self.line(&format!("(void)({});", body_value.code));
        }
        self.indent -= 1;
        self.line("}");
        if has_break {
            let label = self.lexical_loop_label(&target, false)?;
            self.label(&label);
        }
        Ok(())
    }
}
