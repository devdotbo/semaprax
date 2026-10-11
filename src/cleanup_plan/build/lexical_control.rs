//! Explicit execution context for the selective lexical cleanup profile.
use super::*;
use crate::hir::FunctionExecutionId;
#[cfg(test)]
pub(super) mod capacity;
#[cfg(test)]
pub(super) use capacity::exit_capacity;

#[derive(Clone)]
pub(super) struct LexicalBuildState {
    pub owner: FunctionExecutionId,
    pub certificate: super::super::LexicalControlPlan,
    returns: Vec<PendingReturn>,
    active_loops: Vec<LoopContext>,
    pub(super) wrapper_breaks: Vec<super::lexical_break::PendingWrapperBreak>,
    pub(super) building_resume: bool,
    pub(super) replicated_statuses: BTreeSet<StatusSourceId>,
    pub(super) status_normal_visits: Vec<u8>,
    pub(super) status_work: usize,
    pub(super) ensure_regions: Vec<Vec<CleanupRegionId>>,
    pub(super) current_ensure: Option<(usize, usize)>,
}

impl LexicalBuildState {
    pub(super) fn owned_iterator_context(
        &self,
        call: &ExpressionId,
    ) -> Result<Option<&LoopContext>, Diagnostic> {
        let mut contexts = self.active_loops.iter().filter(|context| matches!(
            &context.latch, crate::hir::LoopLatchKind::OwnedIteratorStep { next, .. } if next == call
        ));
        let context = contexts.next();
        if contexts.next().is_some() {
            return Err(plan_error(
                "owning iterator argument has ambiguous active latch authority",
            ));
        }
        Ok(context)
    }
}

/// Fresh monomorphic functions and late generic instances supply their actual
/// execution identity before registration. Legacy functions retain their exact
/// construction path and wire schema.
pub(crate) fn build_plan_for_execution(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
    execution: &FunctionExecutionId,
) -> Result<CleanupPlan, Diagnostic> {
    build_with_helpers(program, function, execution, None)
}

pub(super) fn build_with_index<'a>(
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
    execution: &FunctionExecutionId,
    index: &'a crate::loop_calls::byte_writer::ModuleIndex<'a>,
) -> Result<CleanupPlan, Diagnostic> {
    if !index.matches_program(program) {
        return Err(plan_error(
            "construction helper index belongs to another typed program",
        ));
    }
    build_with_helpers(
        program,
        function,
        execution,
        (!index.is_empty()).then_some(index),
    )
}
fn build_with_helpers<'a>(
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
    execution: &FunctionExecutionId,
    index: Option<&'a crate::loop_calls::byte_writer::ModuleIndex<'a>>,
) -> Result<CleanupPlan, Diagnostic> {
    crate::hir::lexical_exit::validate_owner(function, execution)?;
    if crate::hir::lexical_exit::requires(function)? {
        crate::hir::lexical_exit::validate_function(program, function, execution)?;
    }
    let mut builder = match index {
        Some(index) => PlanBuilder::with_index(program, function, index)?,
        None => PlanBuilder::new(program, function)?,
    };
    if crate::hir::lexical_exit::requires(function)? {
        if builder.schema != super::super::CLEANUP_PLAN_SCHEMA_V19 {
            builder.schema = super::super::CLEANUP_PLAN_SCHEMA_V18;
        }
        builder.lexical = Some(Box::new(LexicalBuildState {
            owner: execution.clone(),
            returns: Vec::new(),
            active_loops: Vec::new(),
            wrapper_breaks: Vec::new(),
            building_resume: false,
            replicated_statuses: BTreeSet::new(),
            status_normal_visits: Vec::new(),
            status_work: 0,
            ensure_regions: Vec::new(),
            current_ensure: None,
            certificate: super::super::LexicalControlPlan {
                schema: "semaprax.lexical-control-flow.v1",
                loops: Vec::new(),
                exits: Vec::new(),
                resumes: Vec::new(),
                abrupt_only_regions: Vec::new(),
                function_success_exits: Vec::new(),
                execution_boundaries: Vec::new(),
            },
        }));
    }
    builder.build()
}

#[derive(Clone)]
struct PendingReturn {
    entry: BlockId,
    state: FlowState,
    result: CleanupResultSource,
}

impl PlanBuilder<'_> {
    pub(super) fn flow_is_closed(&self, block: BlockId) -> bool {
        self.blocks[block.0 as usize].terminator.is_some()
    }

    pub(super) fn mark_abrupt_region(&mut self, region: CleanupRegionId) -> Result<(), Diagnostic> {
        let control = self
            .lexical
            .as_mut()
            .ok_or_else(|| plan_error("abrupt region outside lexical profile"))?;
        if self.regions[region.0 as usize].normal_scope_end == UNRESOLVED_EXIT
            && !control.certificate.abrupt_only_regions.contains(&region)
        {
            control.certificate.abrupt_only_regions.push(region);
        }
        Ok(())
    }

    /// Settle exactly the lexical contour being left, in canonical runtime
    /// order. A root return deliberately retains root locals through ensures.
    pub(super) fn settle_lexical_contour(
        &self,
        state: &mut FlowState,
        regions: &[CleanupRegionId],
    ) -> Result<Vec<FinalizeAction>, Diagnostic> {
        let storages: BTreeSet<_> = regions
            .iter()
            .flat_map(|region| self.regions[region.0 as usize].slots.iter().cloned())
            .collect();
        let finalizers = self.finalizers_for(state, |place| storages.contains(&place.storage));
        self.require_completed_lexical_finalizers(&finalizers)?;

        let flags = state
            .live_order
            .iter()
            .chain(
                state
                    .conditional_variants
                    .iter()
                    .flat_map(|variant| variant.cases.iter().flat_map(|(_, flags)| flags)),
            )
            .filter(|flag| storages.contains(&self.leaves[flag].place.storage))
            .copied()
            .collect();
        state.remove(&flags);
        state
            .conditional_variants
            .retain(|variant| !storages.contains(&variant.root.storage));
        Ok(finalizers)
    }

    pub(super) fn lower_lexical_exit(
        &mut self,
        expression: &ResolvedExpr,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        let ResolvedExprKind::LexicalExit {
            kind,
            target,
            value,
        } = &expression.kind
        else {
            return Err(plan_error(
                "lexical exit classifier received an ordinary expression",
            ));
        };
        let owner = &self
            .lexical
            .as_ref()
            .ok_or_else(|| plan_error("lexical exit outside v18 profile"))?
            .owner;
        if *kind != crate::ast::LexicalExitKind::Return
            || *target != crate::hir::ControlTarget::Function(owner.clone())
        {
            return self.lower_lexical_loop_exit(expression, block, state, region);
        }
        let value = value
            .as_deref()
            .ok_or_else(|| plan_error("function return has no value"))?;
        let mut evaluated = self.lower_expr(value, block, state, region)?;
        if self.flow_is_closed(evaluated.block) {
            return Err(plan_error("return operand has no normal completion"));
        }
        let result = if self.result_needs_drop()? {
            let source = evaluated
                .owned_source
                .take()
                .ok_or_else(|| plan_error("owned return operand has no cleanup source"))?;
            let storage = CleanupPlace::whole(StorageId::ProvisionalResult);
            self.transfer(
                evaluated.block,
                expression.id.clone(),
                source,
                storage.clone(),
                &mut evaluated.state,
                true,
            )?;
            CleanupResultSource::Owned { storage }
        } else {
            CleanupResultSource::Scalar {
                expression: value.id.clone(),
            }
        };
        let leaves_regions: Vec<_> = self
            .region_chain(region)
            .into_iter()
            .take_while(|region| *region != CleanupRegionId(0))
            .collect();
        let finalizers = self.settle_lexical_contour(&mut evaluated.state, &leaves_regions)?;
        let resume = self.new_block(CleanupRegionId(0))?;
        let edge = self.new_edge(evaluated.block, resume, EdgeCondition::Always)?;
        let exit = self.emit_exit(
            evaluated.block,
            leaves_regions,
            finalizers,
            ExitContinuation::FunctionReturn {
                target: target.clone(),
                edge,
                source: result.clone(),
            },
        )?;
        let control = self.lexical.as_mut().expect("lexical profile retained");
        control
            .certificate
            .exits
            .push(super::super::LexicalExitCertificate {
                site: expression.id.clone(),
                target: target.clone(),
                kind: *kind,
                exit,
                resume: Some(resume),
            });
        control.returns.push(PendingReturn {
            entry: resume,
            state: evaluated.state.clone(),
            result,
        });
        evaluated.owned_source = None;
        Ok(evaluated)
    }

    pub(super) fn finish_lexical_returns(&mut self) -> Result<(), Diagnostic> {
        if self
            .lexical
            .as_ref()
            .is_some_and(|control| !control.wrapper_breaks.is_empty())
        {
            return Err(plan_error("owning Break wrapper resume was not completed"));
        }
        let pending = self
            .lexical
            .as_mut()
            .map(|control| std::mem::take(&mut control.returns))
            .unwrap_or_default();
        for returned in pending {
            self.lexical
                .as_mut()
                .expect("lexical profile retained")
                .building_resume = true;
            let mut current = returned.entry;
            let first_new = self.blocks.len();
            let mut state = returned.state;
            for (ordinal, contract) in self.function.ensures.iter().enumerate() {
                (current, state) = self.lower_contract_expression(
                    contract,
                    current,
                    state,
                    CleanupRegionId(0),
                    ContractPhase::Ensures,
                    ordinal,
                )?;
            }
            self.finish_success(current, state, CleanupRegionId(0), returned.result)?;
            let mut blocks = vec![returned.entry];
            blocks.extend(self.blocks[first_new..].iter().map(|block| block.id));
            self.lexical
                .as_mut()
                .expect("lexical profile retained")
                .certificate
                .resumes
                .push(super::super::ResumeCertificate {
                    entry: returned.entry,
                    blocks,
                });
        }
        if let Some(control) = &mut self.lexical {
            control.building_resume = false;
        }
        Ok(())
    }
}

#[derive(Clone)]
pub(super) struct LoopContext {
    pub(super) target: crate::hir::ControlTarget,
    pub(super) region: CleanupRegionId,
    pub(super) body: ExpressionId,
    pub(super) latch: crate::hir::LoopLatchKind,
    pub(super) entry: FlowState,
    pub(super) after: BlockId,
}

impl PlanBuilder<'_> {
    pub(super) fn lower_lexical_while(
        &mut self,
        condition: &ResolvedExpr,
        body: &ResolvedExpr,
        latch: &crate::hir::LoopLatchKind,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        crate::hir::lexical_exit::validate_latch(condition, body, latch)?;
        let control = self
            .lexical
            .as_ref()
            .ok_or_else(|| plan_error("lexical loop outside v18 profile"))?;
        let target = crate::hir::ControlTarget::Loop {
            owner: control.owner.clone(),
            site: condition.id.clone(),
        };
        let condition_entry = block;
        let condition_region = if crate::string_ops::conditions::needs_cleanup(condition) {
            Some(self.new_region(region)?)
        } else {
            None
        };
        let (condition_block, active_region) = if let Some(child) = condition_region {
            let entry = self.new_block(child)?;
            let edge = self.new_edge(block, entry, EdgeCondition::Always)?;
            self.terminate(block, CleanupTerminator::Goto(edge))?;
            (entry, child)
        } else {
            (block, region)
        };
        let mut evaluated_condition =
            self.lower_expr(condition, condition_block, state.clone(), active_region)?;
        if let Some(child) = condition_region {
            (evaluated_condition.block, evaluated_condition.state) =
                self.exit_scope(evaluated_condition.block, evaluated_condition.state, child)?;
        }
        if evaluated_condition.state != state || evaluated_condition.owned_source.is_some() {
            return Err(plan_error("lexical loop condition changes entry ownership"));
        }
        let body_entry = self.new_block(region)?;
        let after = self.new_block(region)?;
        let yes = self.new_edge(
            evaluated_condition.block,
            body_entry,
            EdgeCondition::BooleanResult(condition.id.clone(), true),
        )?;
        let no = self.new_edge(
            evaluated_condition.block,
            after,
            EdgeCondition::BooleanResult(condition.id.clone(), false),
        )?;
        self.terminate(
            evaluated_condition.block,
            CleanupTerminator::Branch(vec![yes, no]),
        )?;
        self.lexical
            .as_mut()
            .expect("lexical profile retained")
            .active_loops
            .push(LoopContext {
                target: target.clone(),
                region,
                body: body.id.clone(),
                latch: latch.clone(),
                entry: state.clone(),
                after,
            });
        self.record_execution_boundary(
            super::super::ExecutionBoundaryKind::LoopIteration,
            &condition.id,
            body_entry,
            &state,
            region,
        )?;
        let evaluated = self.lower_expr(body, body_entry, state.clone(), region)?;
        self.lexical
            .as_mut()
            .expect("lexical profile retained")
            .active_loops
            .pop();
        let iteration_end = if !self.flow_is_closed(evaluated.block) {
            if evaluated.state != state || evaluated.owned_source.is_some() {
                return Err(plan_error(
                    "lexical loop iteration does not restore exact entry ownership",
                ));
            }
            self.emit_exit(
                evaluated.block,
                Vec::new(),
                Vec::new(),
                ExitContinuation::LoopLatch {
                    target: target.clone(),
                },
            )?
        } else {
            UNRESOLVED_EXIT
        };
        self.lexical
            .as_mut()
            .expect("lexical profile retained")
            .certificate
            .loops
            .push(super::super::LoopCertificate {
                target,
                condition: condition.id.clone(),
                body: body.id.clone(),
                latch: latch.clone(),
                region,
                condition_entry,
                latch_entry: None,
                iteration_end,
                after,
                entry_order: state.live_order.clone(),
            });
        Ok(EvalResult {
            block: after,
            state,
            owned_source: None,
        })
    }

    fn lower_lexical_loop_exit(
        &mut self,
        expression: &ResolvedExpr,
        block: BlockId,
        mut state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        let ResolvedExprKind::LexicalExit {
            kind,
            target,
            value: None,
        } = &expression.kind
        else {
            return Err(plan_error("loop exit has an invalid value or target"));
        };
        let context = self
            .lexical
            .as_ref()
            .and_then(|control| control.active_loops.last())
            .cloned()
            .ok_or_else(|| plan_error("loop exit has no nearest active lexical loop"))?;
        if *target != context.target {
            return Err(plan_error("loop exit targets a different lexical contour"));
        }
        let owned_continue = *kind == crate::ast::LexicalExitKind::Continue
            && matches!(
                context.latch,
                crate::hir::LoopLatchKind::OwnedIteratorStep { .. }
            );
        let owned_break = *kind == crate::ast::LexicalExitKind::Break
            && matches!(
                context.latch,
                crate::hir::LoopLatchKind::OwnedIteratorStep { .. }
            );
        let (leaves_regions, finalizers, owned_resume) = if owned_break {
            let (regions, actions, entry) =
                self.lower_owned_iterator_break(&context, &mut state, region)?;
            (regions, actions, Some(entry))
        } else if owned_continue {
            let (regions, actions, entry) =
                self.lower_owned_iterator_continue(&context, &mut state, region)?;
            (regions, actions, Some(entry))
        } else {
            let regions: Vec<_> = self
                .region_chain(region)
                .into_iter()
                .take_while(|region| *region != context.region)
                .collect();
            let actions = self.settle_lexical_contour(&mut state, &regions)?;
            (regions, actions, None)
        };
        let (continuation, resume) = match kind {
            crate::ast::LexicalExitKind::Break => {
                if !owned_break && state != context.entry {
                    return Err(plan_error(
                        "loop break requires exact surrounding ownership state",
                    ));
                }
                let edge = self.new_edge(
                    block,
                    owned_resume.unwrap_or(context.after),
                    EdgeCondition::Always,
                )?;
                (
                    ExitContinuation::LoopBreak {
                        target: target.clone(),
                        edge,
                    },
                    owned_resume,
                )
            }
            crate::ast::LexicalExitKind::Continue => {
                let resume = if let Some(entry) = owned_resume {
                    Some(entry)
                } else {
                    self.lower_continue_advance(&context, &state)?
                };
                if !owned_continue && state != context.entry {
                    return Err(plan_error(
                        "loop continue does not restore exact entry ownership",
                    ));
                }
                (
                    ExitContinuation::LoopLatch {
                        target: target.clone(),
                    },
                    resume,
                )
            }
            crate::ast::LexicalExitKind::Return => {
                return Err(plan_error("return does not target its function execution"))
            }
        };
        let exit = self.emit_exit(block, leaves_regions, finalizers, continuation)?;
        self.lexical
            .as_mut()
            .expect("lexical profile retained")
            .certificate
            .exits
            .push(super::super::LexicalExitCertificate {
                site: expression.id.clone(),
                target: target.clone(),
                kind: *kind,
                exit,
                resume,
            });
        Ok(EvalResult {
            block,
            state,
            owned_source: None,
        })
    }
}

impl PlanBuilder<'_> {
    pub(super) fn lower_loop_statement(
        &mut self,
        statement: &ResolvedStatement,
        block: BlockId,
        state: FlowState,
        region: CleanupRegionId,
    ) -> Result<EvalResult, Diagnostic> {
        match statement {
            ResolvedStatement::While {
                condition, body, ..
            } => self.lower_while(condition, body, block, state, region),
            ResolvedStatement::LexicalWhile {
                condition,
                body,
                latch,
                ..
            } => self.lower_lexical_while(condition, body, latch, block, state, region),
            _ => Err(plan_error("loop classifier received a non-loop statement")),
        }
    }
    pub(super) fn join_normal_branches(
        &mut self,
        mut branches: Vec<EvalResult>,
        region: CleanupRegionId,
        destination: Option<CleanupPlace>,
    ) -> Result<EvalResult, Diagnostic> {
        let abrupt = branches
            .iter()
            .find(|result| self.flow_is_closed(result.block))
            .cloned();
        branches.retain(|result| !self.flow_is_closed(result.block));
        let Some(first) = branches.first() else {
            return abrupt.ok_or_else(|| plan_error("control expression has no reachable branch"));
        };
        let mut state = first.state.clone();
        for branch in branches.iter().skip(1) {
            state = self.merge_states(&state, &branch.state)?;
        }
        let join = self.new_block(region)?;
        for branch in branches {
            let edge = self.new_edge(branch.block, join, EdgeCondition::Always)?;
            self.terminate(branch.block, CleanupTerminator::Goto(edge))?;
        }
        Ok(EvalResult {
            block: join,
            state,
            owned_source: destination,
        })
    }
}

impl PlanBuilder<'_> {
    pub(super) fn normal_branch_inventory(
        &self,
        mut branches: Vec<EvalResult>,
        results: &mut Vec<EvalResult>,
    ) -> Option<Vec<EvalResult>> {
        let abrupt = branches
            .iter()
            .find(|branch| self.flow_is_closed(branch.block))
            .cloned();
        branches.retain(|branch| !self.flow_is_closed(branch.block));
        if branches.is_empty() {
            if let Some(abrupt) = abrupt {
                results.push(abrupt);
            }
            None
        } else {
            Some(branches)
        }
    }
}

pub(super) fn build_registered_plan(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<CleanupPlan, Diagnostic> {
    if crate::hir::lexical_exit::requires(function)? {
        let owner = crate::hir::lexical_exit::owner(program, function)?;
        build_plan_for_execution(program, function, &owner)
    } else {
        PlanBuilder::new(program, function)?.build()
    }
}

/// Locate an exact authenticated identity with a fixed-depth cursor, without
/// cloning HIR or constructing an unbounded expression frontier.
pub(super) fn expression_at<'e>(
    root: &'e ResolvedExpr,
    id: &ExpressionId,
) -> Result<&'e ResolvedExpr, Diagnostic> {
    super::lexical_iterator::lookup::expression_at(root, id)
}
impl PlanBuilder<'_> {
    fn lower_continue_advance(
        &mut self,
        context: &LoopContext,
        state: &FlowState,
    ) -> Result<Option<BlockId>, Diagnostic> {
        match &context.latch {
            crate::hir::LoopLatchKind::Condition => Ok(None),
            crate::hir::LoopLatchKind::SourceForStep { step, .. } => {
                let function = self.function;
                let body = expression_at(&function.body, &context.body)?;
                let ResolvedExprKind::Block { statements, .. } = &body.kind else {
                    return Err(plan_error("source-for latch body is not a block"));
                };
                let Some(ResolvedStatement::Assign {
                    value, field: None, ..
                }) = statements.last()
                else {
                    return Err(plan_error(
                        "source-for latch has no whole counter assignment",
                    ));
                };
                if &value.id != step {
                    return Err(plan_error("source-for advance identity differs"));
                }
                let entry = self.new_block(context.region)?;
                let first_new = self.blocks.len();
                self.lexical
                    .as_mut()
                    .expect("lexical profile retained")
                    .building_resume = true;
                let evaluated = self.lower_expr(value, entry, state.clone(), context.region)?;
                self.lexical
                    .as_mut()
                    .expect("lexical profile retained")
                    .building_resume = false;
                if self.flow_is_closed(evaluated.block)
                    || evaluated.state != context.entry
                    || evaluated.owned_source.is_some()
                {
                    return Err(plan_error(
                        "source-for continue advancement does not restore exact loop ownership",
                    ));
                }
                self.emit_exit(
                    evaluated.block,
                    Vec::new(),
                    Vec::new(),
                    ExitContinuation::LoopLatch {
                        target: context.target.clone(),
                    },
                )?;
                let mut blocks = vec![entry];
                blocks.extend(self.blocks[first_new..].iter().map(|block| block.id));
                self.lexical
                    .as_mut()
                    .expect("lexical profile retained")
                    .certificate
                    .resumes
                    .push(super::super::ResumeCertificate { entry, blocks });
                Ok(Some(entry))
            }
            crate::hir::LoopLatchKind::OwnedIteratorStep { .. } => Err(plan_error(
                "owned iterator continue requires its retained remainder advancement contour",
            )),
        }
    }
}

impl PlanBuilder<'_> {
    pub(super) fn note_resume_status(&mut self, id: &StatusSourceId) {
        if let Some(control) = &mut self.lexical {
            if control.building_resume {
                control.replicated_statuses.insert(id.clone());
            }
        }
    }
}

impl PlanBuilder<'_> {
    /// A generated advancement completes a scope already entered by the
    /// authored loop. Its exact route is selected by the resume certificate;
    /// it never overwrites the ordinary source projection's scope endpoint.
    pub(super) fn record_lexical_scope_end(
        &mut self,
        region: CleanupRegionId,
        exit: ExitTargetId,
    ) -> Result<(), Diagnostic> {
        if let Some(control) = &self.lexical {
            if control.building_resume {
                let contract_scope = control
                    .ensure_regions
                    .iter()
                    .any(|regions| regions.contains(&region));
                if !contract_scope
                    || self.regions[region.0 as usize].normal_scope_end != UNRESOLVED_EXIT
                {
                    return Ok(());
                }
            }
        }
        let normal_end = &mut self.regions[region.0 as usize].normal_scope_end;
        if *normal_end != UNRESOLVED_EXIT {
            return Err(plan_error("cleanup region has multiple normal scope ends"));
        }
        *normal_end = exit;
        Ok(())
    }
}
