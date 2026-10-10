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
    fn lexical_loop_index(&self, target: &ControlTarget) -> Result<usize, Diagnostic> {
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
        let Some(control) = self.function.cleanup_plan.lexical_control.as_deref() else {
            return Ok(());
        };
        for certificate in &control.loops {
            if (matches!(&certificate.latch, LoopLatchKind::SourceForStep { step, .. } if step == site)
                || matches!(&certificate.latch, LoopLatchKind::OwnedIteratorStep { next, .. } if next == site))
                && control.exits.iter().any(|exit| {
                    exit.target == certificate.target && exit.kind == LexicalExitKind::Continue
                })
            {
                let label = self.lexical_loop_label(&certificate.target, true)?;
                self.label(&label);
            }
        }
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
            if let Some(plan) = self.bytes_plan {
                for line in plan.apply_at(&expression.id)?.lines() {
                    self.line(line);
                }
            }
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
            LexicalExitKind::Return => self.line("goto spx_postconditions;"),
            LexicalExitKind::Continue | LexicalExitKind::Break => {
                let label = self.lexical_loop_label(target, kind == LexicalExitKind::Continue)?;
                self.line(&format!("goto {label};"));
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
        let condition_latch =
            certificate.is_some_and(|item| item.latch == LoopLatchKind::Condition);
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
        let has_break = references(LexicalExitKind::Break);
        self.line("for (;;) {");
        self.indent += 1;
        if condition_latch && has_continue {
            let label = self.lexical_loop_label(&target, true)?;
            self.label(&label);
        }
        let condition_value = self.emit_expr(condition)?;
        self.require_type(&condition_value.ty, &ResolvedType::Bool, "while condition")?;
        self.emit_scalar_match_guard_scope_exit(condition)?;
        self.line(&format!("if (!({})) break;", condition_value.code));
        self.semantic_charge();
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
