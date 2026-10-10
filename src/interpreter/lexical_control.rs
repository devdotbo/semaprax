//! Runtime control is distinct from values and failures. Targets and cleanup
//! routes come only from the independently validated selective certificate.
use super::*;
use crate::cleanup_plan::{ExitContinuation, FinalizeAction, StorageId};
use crate::hir::{ControlTarget, FunctionExecutionId, LexicalExitKind};

#[derive(Debug)]
pub(super) enum ExitFlow {
    Break(ControlTarget),
    Continue(ControlTarget),
    Return {
        target: ControlTarget,
        value: Value,
        resume: Option<crate::cleanup_plan::BlockId>,
    },
}

pub(super) struct FrameControl {
    pub(super) owner: FunctionExecutionId,
    pub(super) body: hir::ExpressionId,
    pub(super) resume: Option<crate::cleanup_plan::BlockId>,
}

pub(super) fn target_owner(target: &ControlTarget) -> &FunctionExecutionId {
    match target {
        ControlTarget::Function(owner) | ControlTarget::Loop { owner, .. } => owner,
    }
}
fn execution_key(owner: &FunctionExecutionId) -> &str {
    match owner {
        FunctionExecutionId::Monomorphic(id) => id.as_str(),
        FunctionExecutionId::Generic(id) => id.as_str(),
    }
}

pub(super) fn frame_control(
    function: &ResolvedFunction,
) -> Result<Option<Box<FrameControl>>, Flow> {
    let Some(plan) = function.cleanup_plan.lexical_control.as_deref() else {
        return Ok(None);
    };
    let target = plan
        .exits
        .first()
        .map(|exit| &exit.target)
        .or_else(|| plan.loops.first().map(|loop_| &loop_.target))
        .ok_or(Flow::Guard("lexical profile has no control owner"))?;
    if plan
        .exits
        .iter()
        .any(|exit| target_owner(&exit.target) != target_owner(target))
        || plan
            .loops
            .iter()
            .any(|loop_| target_owner(&loop_.target) != target_owner(target))
    {
        return Err(Flow::Guard("lexical profile mixes function executions"));
    }
    Ok(Some(Box::new(FrameControl {
        owner: target_owner(target).clone(),
        body: function.body.id.clone(),
        resume: None,
    })))
}

impl<'a> Evaluator<'a> {
    pub(super) fn lexical_function(
        &self,
        environment: &Environment,
    ) -> Result<&'a ResolvedFunction, Flow> {
        let context = environment
            .control
            .as_deref()
            .ok_or(Flow::Guard("lexical exit has no live function frame"))?;
        let function = self
            .admitted
            .get(execution_key(&context.owner))
            .ok_or(Flow::Guard(
                "lexical execution is outside the admitted function inventory",
            ))?;
        if function.body.id != context.body {
            return Err(Flow::Guard("lexical function frame identity differs"));
        }
        Ok(function)
    }

    pub(super) fn evaluate_lexical_exit(
        &mut self,
        expression: &ResolvedExpr,
        kind: LexicalExitKind,
        target: &ControlTarget,
        operand: Option<&ResolvedExpr>,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<Value, Flow> {
        let function = self.lexical_function(environment)?;
        let (certificate, route) =
            function
                .cleanup_plan
                .lexical_exit(&expression.id)
                .ok_or(Flow::Guard(
                    "lexical exit lacks its exact cleanup certificate",
                ))?;
        if certificate.target != *target
            || certificate.kind != kind
            || target_owner(target) != &environment.control.as_deref().unwrap().owner
            || expression.ty != ResolvedType::Never
        {
            return Err(Flow::Guard(
                "lexical expression and cleanup certificate disagree",
            ));
        }
        let continuation = match (kind, &route.continuation, operand) {
            (
                LexicalExitKind::Break,
                ExitContinuation::LoopBreak {
                    target: checked, ..
                },
                None,
            ) if checked == target => None,
            (LexicalExitKind::Continue, ExitContinuation::LoopLatch { target: checked }, None)
                if checked == target =>
            {
                None
            }
            (
                LexicalExitKind::Return,
                ExitContinuation::FunctionReturn {
                    target: checked, ..
                },
                Some(value),
            ) if checked == target => Some(value),
            _ => {
                return Err(Flow::Guard(
                    "lexical exit has a mismatched typed continuation",
                ))
            }
        };
        // Operand failure retains the original failure channel and its existing
        // epoch cleanup. A successful owned operand has already left its source.
        let result = match continuation {
            Some(value) => Some(self.evaluate(value, environment, depth)?),
            None => None,
        };
        settle_actions(environment, &route.finalize_in_order, None)?;
        match kind {
            LexicalExitKind::Break => Err(Flow::Lexical(ExitFlow::Break(target.clone()))),
            LexicalExitKind::Continue => {
                let previous = environment.control.as_deref().unwrap().resume;
                environment.control.as_deref_mut().unwrap().resume = certificate.resume;
                let outcome = self.execute_lexical_latch(target, environment, depth);
                environment.control.as_deref_mut().unwrap().resume = previous;
                outcome?;
                Err(Flow::Lexical(ExitFlow::Continue(target.clone())))
            }
            LexicalExitKind::Return => Err(Flow::Lexical(ExitFlow::Return {
                target: target.clone(),
                value: result.ok_or(Flow::Guard("return operand is absent"))?,
                resume: certificate.resume,
            })),
        }
    }
}

pub(super) fn settle_actions(
    environment: &mut Environment,
    actions: &[FinalizeAction],
    mut provisional: Option<&mut Value>,
) -> Result<(), Flow> {
    for action in actions {
        if let Some(active) = &action.active_case {
            let guard = match &active.storage {
                StorageId::Value(guard) => environment.get(guard),
                StorageId::ProvisionalResult => provisional.as_deref(),
                _ => return Err(Flow::Guard("lexical case guard is outside settled storage")),
            };
            match guard {
                Some(Value::Variant(value)) if value.variant == active.variant => {
                    if value.case != active.case {
                        continue;
                    }
                }
                Some(Value::Moved) => continue,
                _ => return Err(Flow::Guard("lexical finalizer case identity differs")),
            }
        }
        let mut value = match &action.source.storage {
            StorageId::Value(root) => environment
                .get_mut(root)
                .ok_or(Flow::Guard("lexical finalizer binding is absent"))?,
            StorageId::ProvisionalResult => provisional.as_deref_mut().ok_or(Flow::Guard(
                "lexical success attempted to finalize its protected result",
            ))?,
            _ => {
                return Err(Flow::Guard(
                    "lexical finalizer references an unsettled temporary",
                ))
            }
        };
        if matches!(value, Value::Moved) {
            continue;
        }
        let mut path = action.source.projections.iter();
        while let Some(field) = path.next() {
            if matches!(value, Value::Moved) {
                break;
            }
            value = match value {
                Value::Record(record) => {
                    Arc::get_mut(record).and_then(|record| record.fields.get_mut(field))
                }
                Value::Variant(variant) => {
                    // Conditional payload paths include the exact case segment
                    // before their field. It is metadata, not a payload field.
                    if &variant.case != field {
                        return Err(Flow::Guard("lexical finalizer payload case differs"));
                    }
                    let field = path
                        .next()
                        .ok_or(Flow::Guard("lexical finalizer payload field is absent"))?;
                    Arc::get_mut(variant).and_then(|variant| variant.fields.get_mut(field))
                }
                _ => return Err(Flow::Guard("lexical finalizer field path differs")),
            }
            .ok_or(Flow::Guard(
                "lexical finalizer field is unavailable or aliased",
            ))?;
        }
        if !matches!(value, Value::Moved) {
            // Ordered replacement physically releases this exact leaf before
            // the next canonical action. Never sort or rebuild the action list.
            drop(std::mem::replace(value, Value::Moved));
        }
    }
    Ok(())
}

impl<'a> Evaluator<'a> {
    fn execute_lexical_latch(
        &mut self,
        target: &ControlTarget,
        environment: &mut Environment,
        depth: usize,
    ) -> Result<(), Flow> {
        let ControlTarget::Loop { site, .. } = target else {
            return Err(Flow::Guard("continue target is not a loop"));
        };
        let function = self.lexical_function(environment)?;
        let certificate = function
            .cleanup_plan
            .lexical_loop(site)
            .ok_or(Flow::Guard("continue has no exact loop certificate"))?;
        if &certificate.target != target {
            return Err(Flow::Guard("continue loop target differs"));
        }
        match &certificate.latch {
            hir::LoopLatchKind::Condition => Ok(()),
            hir::LoopLatchKind::SourceForStep { kind, step } => {
                if !matches!(
                    kind,
                    hir::SourceForKind::Copy | hir::SourceForKind::BorrowRecord
                ) {
                    return Err(Flow::Guard(
                        "own iterator requires its distinct typed latch",
                    ));
                }
                let (binding, value) = find_assignment(&function.body, step)?;
                let value = self.evaluate(value, environment, depth)?;
                let slot = environment
                    .get_mut(&binding.id)
                    .ok_or(Flow::Guard("source For latch target is unavailable"))?;
                *slot = value;
                Ok(())
            }
            hir::LoopLatchKind::OwnedIteratorStep {
                next,
                assignment,
                step,
                remainder,
            } => {
                let (binding, value) = find_assignment(&function.body, assignment)?;
                if &binding.id != step {
                    return Err(Flow::Guard("owned latch Step target differs"));
                }
                let ResolvedExprKind::Match { arms, .. } = &value.kind else {
                    return Err(Flow::Guard("owned latch assignment is not a Step match"));
                };
                let [_, yielded] = arms.as_slice() else {
                    return Err(Flow::Guard("owned latch case inventory differs"));
                };
                let hir::ResolvedMatchPattern::Variant { fields, .. } = &yielded.pattern else {
                    return Err(Flow::Guard("owned latch Yield pattern differs"));
                };
                let [_, rest] = fields.as_slice() else {
                    return Err(Flow::Guard("owned latch Yield fields differ"));
                };
                let ResolvedExprKind::Block { tail, .. } = &yielded.value.kind else {
                    return Err(Flow::Guard("owned latch Yield contour differs"));
                };
                if &rest.binding.id != remainder || &tail.id != next {
                    return Err(Flow::Guard(
                        "owned latch remainder or next identity differs",
                    ));
                }
                // Evaluate only the retained Yield tail. The authored body and
                // whole own-match must never execute a second time.
                let replacement = self.evaluate(tail, environment, depth)?;
                // Each handoff has a distinct physical contour even when its
                // expression anchor is shared. The local replacement protects
                // the result while canonical non-result scopes settle.
                self.settle_lexical_transfer_scope(
                    function,
                    environment,
                    &yielded.value.id,
                    &StorageId::Temporary(yielded.value.id.clone()),
                )?;
                self.settle_lexical_transfer_scope(
                    function,
                    environment,
                    &yielded.value.id,
                    &StorageId::Temporary(assignment.clone()),
                )?;
                let slot = environment
                    .get_mut(step)
                    .ok_or(Flow::Guard("owned latch Step slot is unavailable"))?;
                if !matches!(slot, Value::Moved) {
                    return Err(Flow::Guard(
                        "owned latch Step was not consumed by the active match",
                    ));
                }
                *slot = replacement;
                self.settle_lexical_transfer_scope(
                    function,
                    environment,
                    assignment,
                    &StorageId::Value(step.clone()),
                )
            }
        }
    }
}

fn find_assignment<'a>(
    body: &'a ResolvedExpr,
    id: &hir::ExpressionId,
) -> Result<(&'a hir::ResolvedBinding, &'a ResolvedExpr), Flow> {
    // IDs are authenticated globally by HIR. This bounded, allocation-free
    // cursor still refuses duplicates rather than granting authority by name.
    const DEPTH: usize = crate::cache_codec::MAX_DEPTH;
    let mut frames = [(body, 0usize); DEPTH];
    let mut used = 1usize;
    let mut work = crate::cache_codec::MAX_NODES;
    let mut found = None;
    while used != 0 {
        work = work
            .checked_sub(1)
            .ok_or(Flow::Guard("lexical latch lookup work exhausted"))?;
        let (expression, position) = frames[used - 1];
        if position == 0 {
            if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                for statement in statements {
                    work = work
                        .checked_sub(1)
                        .ok_or(Flow::Guard("lexical latch lookup work exhausted"))?;
                    if let ResolvedStatement::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } = statement
                    {
                        if &value.id == id {
                            if found.replace((binding, value)).is_some() {
                                return Err(Flow::Guard(
                                    "lexical latch assignment identity is duplicated",
                                ));
                            }
                        }
                    }
                }
            }
        }
        frames[used - 1].1 += 1;
        // child() can scan a preceding Block/Match prefix; count that work
        // before lookup, retaining existing deterministic child order.
        if matches!(
            expression.kind,
            ResolvedExprKind::Block { .. } | ResolvedExprKind::Match { .. }
        ) {
            work = work
                .checked_sub(
                    position
                        .checked_add(1)
                        .and_then(|n| n.checked_mul(2))
                        .ok_or(Flow::Guard("lexical lookup work overflow"))?,
                )
                .ok_or(Flow::Guard("lexical latch lookup work exhausted"))?;
        }
        match hir::lexical_exit::child(expression, position) {
            Some(child) => {
                if used == DEPTH {
                    return Err(Flow::Guard("lexical latch lookup depth exceeded"));
                }
                frames[used] = (child, 0);
                used += 1;
            }
            None => used -= 1,
        }
    }
    found.ok_or(Flow::Guard("lexical latch assignment is absent"))
}

impl Evaluator<'_> {
    pub(super) fn settle_lexical_epilogue(
        &self,
        function: &ResolvedFunction,
        environment: &mut Environment,
        failure: Option<&hir::ExpressionId>,
        provisional: Option<&mut Value>,
    ) -> Result<(), Flow> {
        let Some(control) = environment.control.as_deref() else {
            return Ok(());
        };
        let plan = &function.cleanup_plan;
        let mut routes = plan.exits.iter().filter(|exit| {
            plan.lexical_block_in_context(control.resume, exit.from)
                && match (&exit.continuation, failure) {
                    (ExitContinuation::CommitResult { .. }, None) => true,
                    (ExitContinuation::ReturnFailure { source }, Some(expression)) => {
                        &source.expression == expression
                            && source.lane == crate::cleanup_plan::StatusLane::ContractFalse
                    }
                    _ => false,
                }
        });
        let route = routes
            .next()
            .ok_or(Flow::Guard("lexical epilogue has no exact proof route"))?;
        if routes.next().is_some() {
            return Err(Flow::Guard(
                "lexical epilogue repeats its exact proof route",
            ));
        }
        settle_actions(environment, &route.finalize_in_order, provisional)
    }
}

impl Evaluator<'_> {
    fn settle_lexical_transfer_scope(
        &self,
        function: &ResolvedFunction,
        environment: &mut Environment,
        at: &hir::ExpressionId,
        destination: &StorageId,
    ) -> Result<(), Flow> {
        let resume = environment
            .control
            .as_deref()
            .and_then(|control| control.resume);
        let plan = &function.cleanup_plan;
        let mut blocks = plan.blocks.iter().filter(|block| {
            plan.lexical_block_in_context(resume, block.id) && block.transitions.iter().any(|transition| matches!(transition,
                crate::cleanup_plan::CleanupTransition::Transfer { at: anchor, destination: place, .. }
                | crate::cleanup_plan::CleanupTransition::TransferVariant { at: anchor, destination: place, .. }
                | crate::cleanup_plan::CleanupTransition::Renew { at: anchor, destination: place, .. }
                if anchor == at && &place.storage == destination))
        });
        let block = blocks
            .next()
            .ok_or(Flow::Guard("lexical transfer phase has no exact block"))?;
        if blocks.next().is_some() {
            return Err(Flow::Guard("lexical transfer phase repeats its block"));
        }
        let mut exits = plan.exits.iter().filter(|exit| {
            exit.from == block.id
                && exit.leaves_regions.as_slice() == [block.region]
                && matches!(exit.continuation, ExitContinuation::Continue(_))
        });
        let exit = exits.next().ok_or(Flow::Guard(
            "lexical transfer phase lacks canonical scope completion",
        ))?;
        if exits.next().is_some() {
            return Err(Flow::Guard(
                "lexical transfer scope completion is duplicated",
            ));
        }
        settle_actions(environment, &exit.finalize_in_order, None)
    }
}

impl Evaluator<'_> {
    pub(super) fn settle_execution_boundary(
        &self,
        function: &ResolvedFunction,
        environment: &mut Environment,
        kind: crate::cleanup_plan::ExecutionBoundaryKind,
        site: &hir::ExpressionId,
    ) -> Result<(), Flow> {
        if function.cleanup_plan.lexical_control.is_none() {
            return Ok(());
        }
        let resume = environment
            .control
            .as_deref()
            .and_then(|control| control.resume);
        let boundary = function
            .cleanup_plan
            .execution_boundary(kind, site, resume)
            .ok_or(Flow::Guard("execution guard has no exact cleanup boundary"))?;
        settle_actions(environment, &boundary.finalize_in_order, None)
    }
}
