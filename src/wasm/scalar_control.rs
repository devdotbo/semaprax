//! Typed lexical labels for the ordinary scalar ABI. No Never local/result.
use super::*;
use crate::cleanup_plan::LoopCertificate;
use crate::hir::{ControlTarget, LexicalExitKind, LoopLatchKind, ResolvedFunction};

#[derive(Clone, Copy)]
struct LoopLabel<'a> {
    certificate: &'a LoopCertificate,
    break_depth: u32,
    continue_depth: u32,
}

pub(super) struct Control<'a> {
    function: Option<&'a ResolvedFunction>,
    pub(super) depth: u32,
    return_depth: Option<u32>,
    result_local: u32,
    loops: Vec<LoopLabel<'a>>,
}
impl<'a> Control<'a> {
    pub(super) fn legacy() -> Self {
        Self {
            function: None,
            depth: 0,
            return_depth: None,
            result_local: 0,
            loops: Vec::new(),
        }
    }
    pub(super) fn enter_loop(&mut self, condition: &ResolvedExpr) -> Result<(), Diagnostic> {
        let Some(function) = self.function else {
            return Ok(());
        };
        let certificate = function
            .cleanup_plan
            .lexical_loop(&condition.id)
            .ok_or_else(|| failure("loop has no exact lexical certificate"))?;
        if self.loops.len() == crate::cache_codec::MAX_DEPTH {
            return Err(failure("lexical label depth exceeded"));
        }
        if self.loops.len() == self.loops.capacity() {
            let capacity = self
                .loops
                .capacity()
                .max(1)
                .checked_mul(2)
                .ok_or_else(|| failure("lexical label capacity overflow"))?
                .min(crate::cache_codec::MAX_DEPTH);
            if !crate::bounded_output::reserve_active_required(
                capacity * std::mem::size_of::<LoopLabel<'_>>(),
            ) {
                return Err(failure("lexical label scratch exceeds output reservation"));
            }
            let mut next = Vec::with_capacity(capacity);
            next.extend_from_slice(&self.loops);
            self.loops = next;
        }
        self.loops.push(LoopLabel {
            certificate,
            break_depth: self.depth - 1,
            continue_depth: self.depth,
        });
        Ok(())
    }
    pub(super) fn leave_loop(&mut self) {
        if self.function.is_some() {
            self.loops.pop();
        }
    }

    pub(super) fn emit_exit(
        &mut self,
        output: &mut impl ByteOutput,
        expression: &ResolvedExpr,
        kind: LexicalExitKind,
        target: &ControlTarget,
        operand: Option<&ResolvedExpr>,
        value_indexes: &HashMap<ValueId, u32>,
        function_indexes: &HashMap<FunctionExecutionId, u32>,
        layout: &LocalLayout,
        result: Option<(u32, &str)>,
    ) -> Result<(), Diagnostic> {
        let function = self
            .function
            .ok_or_else(|| failure("exit has no lexical function context"))?;
        let (certificate, route) = function
            .cleanup_plan
            .lexical_exit(&expression.id)
            .ok_or_else(|| failure("exit lacks its exact cleanup certificate"))?;
        if certificate.kind != kind
            || &certificate.target != target
            || expression.ty != ResolvedType::Never
        {
            return Err(failure("exit certificate and resolved expression disagree"));
        }
        // Resource carriers always use the existing owned/aggregate planner,
        // whose canonical guarded cleanup emitter owns their physical release.
        if !route.finalize_in_order.is_empty() {
            return Err(failure(
                "owned lexical exit reached the scalar carrier lane",
            ));
        }
        let label = match (kind, &route.continuation, operand) {
            (
                LexicalExitKind::Return,
                crate::cleanup_plan::ExitContinuation::FunctionReturn {
                    target: checked, ..
                },
                Some(value),
            ) if checked == target => {
                scalar_emit::emit_with_control(
                    output,
                    value,
                    value_indexes,
                    function_indexes,
                    layout,
                    result,
                    self,
                )?;
                local_set(output, self.result_local);
                self.return_depth
                    .ok_or_else(|| failure("return epilogue label is absent"))?
            }
            (
                LexicalExitKind::Break,
                crate::cleanup_plan::ExitContinuation::LoopBreak {
                    target: checked, ..
                },
                None,
            ) if checked == target => {
                self.loops
                    .last()
                    .filter(|label| &label.certificate.target == target)
                    .ok_or_else(|| failure("break target is not the nearest bound loop"))?
                    .break_depth
            }
            (
                LexicalExitKind::Continue,
                crate::cleanup_plan::ExitContinuation::LoopLatch { target: checked },
                None,
            ) if checked == target => {
                let label = *self
                    .loops
                    .last()
                    .filter(|label| &label.certificate.target == target)
                    .ok_or_else(|| failure("continue target is not the nearest bound loop"))?;
                match &label.certificate.latch {
                    LoopLatchKind::Condition => {}
                    LoopLatchKind::SourceForStep {
                        kind: crate::hir::SourceForKind::Copy,
                        step,
                    } => {
                        let (binding, value) = lexical_lookup::assignment(&function.body, step)?;
                        scalar_emit::emit_with_control(
                            output,
                            value,
                            value_indexes,
                            function_indexes,
                            layout,
                            result,
                            self,
                        )?;
                        local_set(
                            output,
                            *layout
                                .lets
                                .get(&binding.id)
                                .ok_or_else(|| failure("For latch target local is absent"))?,
                        );
                    }
                    _ => return Err(failure("owned/record For latch reached the scalar lane")),
                }
                label.continue_depth
            }
            _ => return Err(failure("exit typed continuation differs")),
        };
        output.push(0x0c);
        write_u32(
            output,
            self.depth
                .checked_sub(label)
                .ok_or_else(|| failure("exit label is outside lexical depth"))?,
        );
        Ok(())
    }
}

pub(super) fn emit_body(
    output: &mut impl ByteOutput,
    function: &ResolvedFunction,
    value_indexes: &HashMap<ValueId, u32>,
    function_indexes: &HashMap<FunctionExecutionId, u32>,
    layout: &LocalLayout,
    result_local: u32,
) -> Result<(), Diagnostic> {
    if function.cleanup_plan.lexical_control.is_none() {
        scalar_emit::emit_expr(
            output,
            &function.body,
            value_indexes,
            function_indexes,
            layout,
            None,
        )?;
        local_set(output, result_local);
        return Ok(());
    }
    // With no owned storage and no finalizers, every authenticated return
    // contour has the same scalar postcondition/publication behavior. This
    // lane emits no cleanup transitions or recoverable status carrier, so a
    // private discriminator would carry no distinct runtime operation.
    if !function.cleanup_plan.slots.is_empty()
        || function
            .cleanup_plan
            .exits
            .iter()
            .any(|exit| !exit.finalize_in_order.is_empty())
    {
        return Err(failure(
            "owned lexical epilogue requires the aggregate lane",
        ));
    }
    let mut control = Control {
        function: Some(function),
        depth: 1,
        return_depth: Some(1),
        result_local,
        loops: Vec::new(),
    };
    output.extend_bytes(&[0x02, 0x40]);
    scalar_emit::emit_with_control(
        output,
        &function.body,
        value_indexes,
        function_indexes,
        layout,
        None,
        &mut control,
    )?;
    if function.body.ty != ResolvedType::Never {
        local_set(output, result_local);
    }
    output.push(0x0b);
    Ok(())
}
fn failure(detail: &str) -> Diagnostic {
    Diagnostic::io("SPX-W110", format!("lexical scalar lowering: {detail}"))
}
