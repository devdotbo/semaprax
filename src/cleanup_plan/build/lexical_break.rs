//! An owning source-For Break finishes its wrapper, never the live Step path.
use super::lexical_control::LoopContext;
use super::*;

#[derive(Clone)]
pub(super) struct PendingWrapperBreak {
    entry: BlockId,
    wrapper: ExpressionId,
    region: CleanupRegionId,
    state: FlowState,
    step: crate::hir::ValueId,
    assignment: ExpressionId,
}

#[cfg(test)]
impl PendingWrapperBreak {
    pub(super) fn owned_capacity(&self) -> usize {
        self.wrapper
            .owned_allocation_bytes()
            .expect("identity census")
            + self
                .assignment
                .owned_allocation_bytes()
                .expect("identity census")
            + self.step.as_str().len()
            + super::lexical_control::capacity::flow_capacity(&self.state)
    }
}

impl PlanBuilder<'_> {
    pub(super) fn lower_owned_iterator_break(
        &mut self,
        context: &LoopContext,
        state: &mut FlowState,
        region: CleanupRegionId,
    ) -> Result<(Vec<CleanupRegionId>, Vec<FinalizeAction>, BlockId), Diagnostic> {
        let crate::hir::LoopLatchKind::OwnedIteratorStep {
            step, assignment, ..
        } = &context.latch
        else {
            return Err(plan_error("wrapper Break has no owning iterator latch"));
        };
        let wrapper = owning_wrapper(self.program, &self.function.body, &context.target)?;
        let proof = crate::hir::lexical_exit::source_for_wrapper(self.program, wrapper)?
            .ok_or_else(|| plan_error("owning Break has no authenticated source-For wrapper"))?;
        if proof.iterator_step.map(|binding| &binding.id) != Some(step)
            || !self.regions[context.region.0 as usize]
                .slots
                .contains(&StorageId::Value(step.clone()))
        {
            return Err(plan_error(
                "owning Break wrapper does not own its exact Step",
            ));
        }
        let chain = self.region_chain(region);
        let Some(index) = chain
            .iter()
            .position(|candidate| *candidate == context.region)
        else {
            return Err(plan_error(
                "owning Break wrapper is outside its active contour",
            ));
        };
        let leaves_regions = chain[..index].to_vec();
        let finalizers = self.settle_lexical_contour(state, &leaves_regions)?;
        // Match consumed the Step before entering Yield. It cannot reappear on
        // this path merely to agree with the loop's false-condition branch.
        let step_storage = StorageId::Value(step.clone());
        if state
            .live_order
            .iter()
            .any(|flag| self.leaves[flag].place.storage == step_storage)
            || state
                .conditional_variants
                .iter()
                .any(|variant| variant.root.storage == step_storage)
        {
            return Err(plan_error(
                "owning Break unexpectedly retains a live hidden Step",
            ));
        }
        let entry = self.new_block(context.region)?;
        self.lexical
            .as_mut()
            .ok_or_else(|| plan_error("owning Break outside lexical profile"))?
            .wrapper_breaks
            .push(PendingWrapperBreak {
                entry,
                wrapper: wrapper.id.clone(),
                region: context.region,
                state: state.clone(),
                step: step.clone(),
                assignment: assignment.clone(),
            });
        Ok((leaves_regions, finalizers, entry))
    }

    /// Called after the ordinary BlockAfterTail has left this wrapper. Each
    /// abrupt predecessor completes independently, then agrees exactly at the
    /// parent join. No merge repairs ownership or initialization history.
    pub(super) fn finish_wrapper_breaks(
        &mut self,
        wrapper: &ResolvedExpr,
        region: CleanupRegionId,
        after: BlockId,
        expected: &FlowState,
    ) -> Result<(), Diagnostic> {
        let Some(control) = &mut self.lexical else {
            return Ok(());
        };
        let mut selected = Vec::new();
        let pending = std::mem::take(&mut control.wrapper_breaks);
        for item in pending {
            if item.region == region {
                if item.wrapper != wrapper.id {
                    return Err(plan_error("owning Break resume names a different wrapper"));
                }
                selected.push(item);
            } else {
                control.wrapper_breaks.push(item);
            }
        }
        if selected.is_empty() {
            return Ok(());
        }
        let proof = crate::hir::lexical_exit::source_for_wrapper(self.program, wrapper)?
            .ok_or_else(|| plan_error("pending Break wrapper lost its source authority"))?;
        let ResolvedExprKind::Block { tail, .. } = &wrapper.kind else {
            unreachable!()
        };
        for pending in selected {
            if proof.iterator_step.map(|binding| &binding.id) != Some(&pending.step) {
                return Err(plan_error(
                    "pending Break Step differs from authenticated wrapper",
                ));
            }
            let first_new = self.blocks.len();
            let previous = self
                .lexical
                .as_ref()
                .expect("lexical profile retained")
                .building_resume;
            self.lexical
                .as_mut()
                .expect("lexical profile retained")
                .building_resume = true;
            let completion = (|| {
                let mut evaluated = self.lower_expr(tail, pending.entry, pending.state, region)?;
                if self.flow_is_closed(evaluated.block) || evaluated.owned_source.is_some() {
                    return Err(plan_error(
                        "owning Break wrapper zero tail did not complete normally",
                    ));
                }
                // This reservation belongs to the exited Step assignment, not
                // to a renewed owner. Cancel it only at the wrapper's leave.
                if evaluated
                    .state
                    .renewals
                    .remove(&pending.assignment)
                    .is_none()
                {
                    return Err(plan_error(
                        "owning Break has no exact Step renewal reservation",
                    ));
                }
                let (block, state) = self.exit_scope(evaluated.block, evaluated.state, region)?;
                if &state != expected {
                    return Err(plan_error(
                        "owning Break wrapper changed exact parent ownership history",
                    ));
                }
                let edge = self.new_edge(block, after, EdgeCondition::Always)?;
                self.terminate(block, CleanupTerminator::Goto(edge))
            })();
            self.lexical
                .as_mut()
                .expect("lexical profile retained")
                .building_resume = previous;
            completion?;
            let mut blocks = vec![pending.entry];
            blocks.extend(self.blocks[first_new..].iter().map(|block| block.id));
            self.lexical
                .as_mut()
                .expect("lexical profile retained")
                .certificate
                .resumes
                .push(super::super::ResumeCertificate {
                    entry: pending.entry,
                    blocks,
                });
        }
        Ok(())
    }
}

/// Fixed-depth, allocation-free source identity lookup. The complete wrapper
/// recognizer independently authenticates its seed and latch after selection.
fn owning_wrapper<'a>(
    program: &ResolvedProgram,
    root: &'a ResolvedExpr,
    target: &crate::hir::ControlTarget,
) -> Result<&'a ResolvedExpr, Diagnostic> {
    let crate::hir::ControlTarget::Loop { site, .. } = target else {
        return Err(plan_error("owning Break does not target a loop"));
    };
    const DEPTH: usize = crate::cache_codec::MAX_DEPTH + 1;
    let mut stack: [Option<(&ResolvedExpr, usize)>; DEPTH] = [None; DEPTH];
    stack[0] = Some((root, 0));
    let mut used = 1;
    let mut work = 0usize;
    let mut found = None;
    while used != 0 {
        work += 1;
        if work > 1_000_000 {
            return Err(plan_error(
                "owning Break wrapper lookup exceeds bounded work",
            ));
        }
        let (expression, index) = stack[used - 1].expect("occupied wrapper lookup frame");
        if index == 0 {
            if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                if matches!(statements.last(), Some(ResolvedStatement::LexicalWhile { condition, .. }) if &condition.id == site)
                {
                    if found.is_some()
                        || crate::hir::lexical_exit::source_for_wrapper(program, expression)?
                            .is_none()
                    {
                        return Err(plan_error(
                            "owning Break wrapper identity is missing or ambiguous",
                        ));
                    }
                    found = Some(expression);
                }
            }
        }
        if let Some(child) = crate::hir::lexical_exit::child(expression, index) {
            stack[used - 1] = Some((expression, index + 1));
            if used == DEPTH {
                return Err(plan_error(
                    "owning Break wrapper lookup exceeds bounded depth",
                ));
            }
            stack[used] = Some((child, 0));
            used += 1;
        } else {
            used -= 1;
            stack[used] = None;
        }
    }
    found.ok_or_else(|| plan_error("owning Break wrapper identity is absent"))
}
