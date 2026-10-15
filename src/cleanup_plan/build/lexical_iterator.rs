//! Finite advancement of an authenticated owning traversal after Continue.
use super::lexical_control::{expression_at, LoopContext};
use super::*;

pub(super) mod lookup;

impl PlanBuilder<'_> {
    /// Retain the structurally required, inactive Done temporary in its authored
    /// arm scope. The exact true-condition proof excludes runtime completion,
    /// not the full typed inventory or its storage-to-region correspondence.
    pub(super) fn prepare_true_iterator_match(
        &mut self,
        expression: &ResolvedExpr,
        parent: CleanupRegionId,
    ) -> Result<usize, Diagnostic> {
        if !self
            .lexical
            .as_ref()
            .is_some_and(|control| control.is_true_iterator_match(&expression.id))
        {
            return Ok(0);
        }
        let ResolvedExprKind::Match {
            mode: ResolvedMatchMode::Own,
            scrutinee,
            arms,
            ..
        } = &expression.kind
        else {
            return Err(plan_error("true iterator body lost its owned match"));
        };
        let [done, _] = arms.as_slice() else {
            return Err(plan_error("true iterator body lost its exact two arms"));
        };
        if done.guard.is_some()
            || done.value.ownership != OwnershipMode::Own
            || done.value.ty != scrutinee.ty
            || !matches!(&done.value.kind, ResolvedExprKind::ConstructVariant { variant, case, fields }
                if variant.as_str() == crate::iterator_ops::STEP_ID
                    && case.as_str() == crate::iterator_ops::DONE_ID && fields.is_empty())
        {
            return Err(plan_error(
                "true iterator Done is not its childless owned constructor",
            ));
        }
        let region = self.new_region(parent)?;
        self.expression_slot(&done.value, region)?;
        self.mark_abrupt_region(region)?;
        // No initialization, transition, normal endpoint or finalizer is minted:
        // the constructor remains structurally present and runtime inactive.
        Ok(1)
    }

    /// The same authored Next is lowered once per finite continuation domain
    /// and once on its ordinary tail. All instances share one typed argument
    /// slot in the original Yield block, not fresh storage or broader authority.
    pub(super) fn reuses_owned_iterator_argument(
        &self,
        storage: &StorageId,
        region: CleanupRegionId,
    ) -> Result<bool, Diagnostic> {
        let StorageId::CallArgument {
            call,
            parameter_index: 0,
            value_expression,
        } = storage
        else {
            return Ok(false);
        };
        let Some(context) = self
            .lexical
            .as_ref()
            .map(|control| control.owned_iterator_context(call))
            .transpose()?
            .flatten()
        else {
            return Ok(false);
        };
        let crate::hir::LoopLatchKind::OwnedIteratorStep {
            next, remainder, ..
        } = &context.latch
        else {
            return Ok(false);
        };
        let expression = expression_at(&self.function.body, next)?;
        let ResolvedExprKind::Call { args, .. } = &expression.kind else {
            return Ok(false);
        };
        let [argument] = args.as_slice() else {
            return Ok(false);
        };
        if &argument.id != value_expression
            || argument.ownership != OwnershipMode::Own
            || !matches!(&argument.kind, ResolvedExprKind::Place(place) if &place.root == remainder && place.projections.is_empty())
        {
            return Ok(false);
        }
        let Some(yield_region) = self
            .regions
            .get(region.0 as usize)
            .filter(|r| r.id == region)
        else {
            return Ok(false);
        };
        let Some(arm_region) = yield_region
            .parent
            .and_then(|id| self.regions.get(id.0 as usize))
            .filter(|r| r.slots.contains(&StorageId::Value(remainder.clone())))
        else {
            return Ok(false);
        };
        let Some(body_region) = arm_region
            .parent
            .and_then(|id| self.regions.get(id.0 as usize))
        else {
            return Ok(false);
        };
        Ok(body_region.parent == Some(context.region))
    }

    /// Keep the entered Yield scopes and remainder until the certified Next
    /// call commits. Authored locals and the unmoved item end before Next.
    pub(super) fn lower_owned_iterator_continue(
        &mut self,
        context: &LoopContext,
        state: &mut FlowState,
        region: CleanupRegionId,
    ) -> Result<(Vec<CleanupRegionId>, Vec<FinalizeAction>, BlockId), Diagnostic> {
        let crate::hir::LoopLatchKind::OwnedIteratorStep {
            next,
            assignment,
            step,
            remainder,
        } = &context.latch
        else {
            return Err(plan_error(
                "owned continuation has no owning iterator latch",
            ));
        };
        let function = self.function;
        let body = expression_at(&function.body, &context.body)?;
        let ResolvedExprKind::Block { statements, .. } = &body.kind else {
            return Err(plan_error(
                "owning traversal body is not its authenticated block",
            ));
        };
        let [ResolvedStatement::Assign {
            binding,
            field: None,
            value,
            ..
        }] = statements.as_slice()
        else {
            return Err(plan_error("owning traversal has no exact Step assignment"));
        };
        let ResolvedExprKind::Match { arms, .. } = &value.kind else {
            return Err(plan_error(
                "owning traversal Step assignment has no exact match",
            ));
        };
        let Some(yield_arm) = arms.get(1) else {
            return Err(plan_error("owning traversal has no Yield arm"));
        };
        let ResolvedExprKind::Block { tail, .. } = &yield_arm.value.kind else {
            return Err(plan_error("owning traversal Yield arm is not a block"));
        };
        let ResolvedMatchPattern::Variant { fields, .. } = &yield_arm.pattern else {
            return Err(plan_error(
                "owning traversal Yield pattern has no retained fields",
            ));
        };
        if &value.id != assignment
            || &tail.id != next
            || &binding.id != step
            || fields.len() != 2
            || &fields[1].binding.id != remainder
        {
            return Err(plan_error(
                "owning traversal continuation differs from the authenticated latch",
            ));
        }
        let chain = self.region_chain(region);
        let remainder_storage = StorageId::Value(remainder.clone());
        let mut owners = chain.iter().enumerate().filter(|(_, candidate)| {
            self.regions[candidate.0 as usize]
                .slots
                .contains(&remainder_storage)
        });
        let (arm_index, arm_region) = owners
            .next()
            .map(|(index, region)| (index, *region))
            .ok_or_else(|| plan_error("owning continuation remainder has no active Yield scope"))?;
        if owners.next().is_some() || arm_index == 0 {
            return Err(plan_error(
                "owning continuation remainder scope is ambiguous",
            ));
        }
        let yield_region = chain[arm_index - 1];
        let body_region = self.regions[arm_region.0 as usize]
            .parent
            .ok_or_else(|| plan_error("owning continuation Yield scope has no body parent"))?;
        if self.regions[yield_region.0 as usize].parent != Some(arm_region)
            || self.regions[body_region.0 as usize].parent != Some(context.region)
        {
            return Err(plan_error(
                "owning continuation retained scope chain differs",
            ));
        }
        let leaves_regions = chain[..arm_index - 1].to_vec();
        let mut disposed: BTreeSet<_> = leaves_regions
            .iter()
            .flat_map(|region| self.regions[region.0 as usize].slots.iter().cloned())
            .collect();
        if fields[0].binding.ownership == OwnershipMode::Own {
            disposed.insert(StorageId::Value(fields[0].binding.id.clone()));
        }
        let finalizers = self.finalizers_for(state, |place| disposed.contains(&place.storage));
        self.require_completed_lexical_finalizers(&finalizers)?;

        let flags: BTreeSet<_> = state
            .live_order
            .iter()
            .chain(
                state
                    .conditional_variants
                    .iter()
                    .flat_map(|variant| variant.cases.iter().flat_map(|(_, flags)| flags)),
            )
            .filter(|flag| disposed.contains(&self.leaves[flag].place.storage))
            .copied()
            .collect();
        state.remove(&flags);
        state
            .conditional_variants
            .retain(|variant| !disposed.contains(&variant.root.storage));

        let entry = self.new_block(yield_region)?;
        let first_new = self.blocks.len();
        self.lexical
            .as_mut()
            .expect("lexical profile retained")
            .building_resume = true;
        let mut advanced = self.lower_expr(tail, entry, state.clone(), yield_region)?;
        if self.flow_is_closed(advanced.block) {
            return Err(plan_error("iterator Next has no ordinary completion"));
        }
        let arm_result = self
            .expression_slot(&yield_arm.value, arm_region)?
            .ok_or_else(|| plan_error("owning continuation Yield block has no result storage"))?;
        let source = advanced
            .owned_source
            .take()
            .ok_or_else(|| plan_error("owning continuation Next has no owned result"))?;
        self.transfer(
            advanced.block,
            yield_arm.value.id.clone(),
            source,
            arm_result.clone(),
            &mut advanced.state,
            true,
        )?;
        (advanced.block, advanced.state) =
            self.exit_scope(advanced.block, advanced.state, yield_region)?;
        advanced.owned_source = Some(arm_result);
        generic_variant::finish_arm(self, value, &yield_arm.value, &mut advanced, body_region)?;
        (advanced.block, advanced.state) =
            self.exit_scope(advanced.block, advanced.state, arm_region)?;
        let destination = self
            .binding_slot(binding, context.region)?
            .ok_or_else(|| plan_error("owning continuation Step has no binding storage"))?;
        let source = advanced
            .owned_source
            .take()
            .ok_or_else(|| plan_error("owning continuation match has no owned result"))?;
        self.transfer(
            advanced.block,
            value.id.clone(),
            source,
            destination,
            &mut advanced.state,
            true,
        )?;
        (advanced.block, advanced.state) =
            self.exit_scope(advanced.block, advanced.state, body_region)?;
        if advanced.state != context.entry {
            return Err(plan_error(
                "owning continuation does not restore exact loop entry state",
            ));
        }
        self.emit_exit(
            advanced.block,
            Vec::new(),
            Vec::new(),
            ExitContinuation::LoopLatch {
                target: context.target.clone(),
            },
        )?;
        self.lexical
            .as_mut()
            .expect("lexical profile retained")
            .building_resume = false;
        let mut blocks = vec![entry];
        blocks.extend(self.blocks[first_new..].iter().map(|block| block.id));
        self.lexical
            .as_mut()
            .expect("lexical profile retained")
            .certificate
            .resumes
            .push(super::super::ResumeCertificate { entry, blocks });
        Ok((leaves_regions, finalizers, entry))
    }
}
