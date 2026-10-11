//! Scalar match continuations live in the existing bounded heap machine.
//! No guard or selected value starts a nested expression evaluator.
use super::*;

pub(super) struct State<'a> {
    expression: &'a ResolvedExpr,
    scrutinee: &'a ResolvedExpr,
    arms: &'a [ResolvedMatchArm],
    index: usize,
    remaining: Vec<ExprSkeletonPath>,
    results: Vec<ExprSkeletonPath>,
}
pub(super) enum Next<'a> {
    Guard(Guard<'a>),
    Value(Value<'a>),
    Done(Vec<ExprSkeletonPath>),
}
pub(super) struct Guard<'a> {
    state: State<'a>,
    selected: Vec<ExprSkeletonPath>,
    rejected: Vec<ExprSkeletonPath>,
}
pub(super) struct Value<'a> {
    state: State<'a>,
    pending: std::vec::IntoIter<ExprSkeletonPath>,
}
impl<'a> State<'a> {
    pub(super) fn new(
        program: &dyn ReplayContext,
        function: &ResolvedFunction,
        expression: &'a ResolvedExpr,
        scrutinee: &'a ResolvedExpr,
        arms: &'a [ResolvedMatchArm],
        remaining: Vec<ExprSkeletonPath>,
    ) -> Result<Self, Diagnostic> {
        if type_needs_drop(program, function, &expression.ty)?
            && !matches!(expression.ty, crate::hir::ResolvedType::String)
        {
            return Err(replay_error(
                function,
                "droppable refutable-match result reached the copy-only cleanup skeleton",
            ));
        }
        let last = arms
            .last()
            .ok_or_else(|| replay_error(function, "refutable match has no arms"))?;
        let catch_all = matches!(
            &last.pattern,
            ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Binding(_)
        );
        if !catch_all || last.guard.is_some() {
            return Err(replay_error(
                function,
                "resolved refutable match lacks a trailing irrefutable guard-free catch-all",
            ));
        }

        Ok(Self {
            expression,
            scrutinee,
            arms,
            index: 0,
            remaining,
            results: Vec::new(),
        })
    }
    pub(super) fn next(mut self, work: &mut SkeletonWork<'_, '_>) -> Result<Next<'a>, Diagnostic> {
        let function = work.function;
        while self.index < self.arms.len() {
            let index = self.index;
            let arm = &self.arms[index];
            let scrutinee = self.scrutinee;
            let final_arm = index + 1 == self.arms.len();
            // Terminal paths bypass the rest of the decision chain.
            let mut active = Vec::new();
            for path in std::mem::take(&mut self.remaining) {
                if path.failed || path.residual {
                    work.push_existing_path(&mut self.results, path, "scalar match path move")?;
                } else {
                    work.push_existing_path(&mut active, path, "scalar match path move")?;
                }
            }

            let mut selected_paths: Vec<ExprSkeletonPath> = Vec::new();
            let mut rejected_paths: Vec<ExprSkeletonPath> = Vec::new();
            if final_arm {
                selected_paths = active;
            } else {
                let arm_index = u32::try_from(index)
                    .map_err(|_| replay_error(function, "too many scalar match arms"))?;
                for path in active {
                    let mut selected =
                        work.clone_expr_path(&path, "scalar match selected clone")?;
                    let scrutinee_id =
                        work.clone_owned(&scrutinee.id, "scalar match selected scrutinee clone")?;
                    work.push_observation(
                        &mut selected,
                        SkeletonObservation::ArmSelected {
                            scrutinee: scrutinee_id,
                            arm: arm_index,
                            selected: true,
                        },
                        "scalar match selected observation",
                    )?;
                    work.push_existing_path(
                        &mut selected_paths,
                        selected,
                        "scalar match path move",
                    )?;

                    let mut rejected = path;
                    let scrutinee_id =
                        work.clone_owned(&scrutinee.id, "scalar match rejected scrutinee clone")?;
                    work.push_observation(
                        &mut rejected,
                        SkeletonObservation::ArmSelected {
                            scrutinee: scrutinee_id,
                            arm: arm_index,
                            selected: false,
                        },
                        "scalar match rejected observation",
                    )?;
                    work.push_existing_path(
                        &mut rejected_paths,
                        rejected,
                        "scalar match path move",
                    )?;
                }
            }

            if arm.guard.is_some() {
                return Ok(Next::Guard(Guard {
                    state: self,
                    selected: selected_paths,
                    rejected: rejected_paths,
                }));
            }
            work.extend_existing_paths(
                &mut self.remaining,
                rejected_paths,
                "scalar match path extension",
            )?;
            let pending = selected_paths.into_iter();
            if !pending.as_slice().is_empty() {
                return Ok(Next::Value(Value {
                    state: self,
                    pending,
                }));
            }
            self.index += 1;
        }
        for path in self.remaining {
            work.push_existing_path(&mut self.results, path, "scalar match path move")?;
        }
        Ok(Next::Done(self.results))
    }
    fn values(
        mut self,
        paths: Vec<ExprSkeletonPath>,
        work: &mut SkeletonWork<'_, '_>,
    ) -> Result<Next<'a>, Diagnostic> {
        let pending = paths.into_iter();
        if !pending.as_slice().is_empty() {
            Ok(Next::Value(Value {
                state: self,
                pending,
            }))
        } else {
            self.index += 1;
            self.next(work)
        }
    }
}
impl<'a> Guard<'a> {
    pub(super) fn expression(&self) -> &'a ResolvedExpr {
        self.state.arms[self.state.index]
            .guard
            .as_deref()
            .expect("scalar guard retained")
    }
    pub(super) fn finish(
        mut self,
        guard_paths: Vec<ExprSkeletonPath>,
        work: &mut SkeletonWork<'_, '_>,
    ) -> Result<Next<'a>, Diagnostic> {
        let guard = self.expression();
        let selected_paths = self.selected;
        let mut rejected_paths = self.rejected;
        let (terminal, when_true, when_false) =
            split_boolean_prefixes_at(guard_paths, &guard.id, work)?;
        // The true continuation consumes the shared prefixes; false and terminal continuations clone them.
        let mut with_false = Vec::new();
        for prefix in &selected_paths {
            let cloned = clone_expr_path_shallow(prefix, work)?;
            let prefixed = sequence_skeleton_paths(
                work.singleton_path(cloned, "guard-false prefix")?,
                &when_false,
                work,
            )?;
            work.extend_existing_paths(&mut with_false, prefixed, "scalar match path extension")?;
        }
        let mut with_terminal = Vec::new();
        for prefix in &selected_paths {
            let cloned = clone_expr_path_shallow(prefix, work)?;
            let prefixed = sequence_skeleton_paths(
                work.singleton_path(cloned, "guard-terminal prefix")?,
                &terminal,
                work,
            )?;
            work.extend_existing_paths(
                &mut with_terminal,
                prefixed,
                "scalar match path extension",
            )?;
        }
        let with_true = sequence_skeleton_paths(selected_paths, &when_true, work)?;
        // False guards and failed guard evaluations fall through.
        work.extend_existing_paths(
            &mut rejected_paths,
            with_false,
            "scalar match path extension",
        )?;
        work.extend_existing_paths(
            &mut rejected_paths,
            with_terminal,
            "scalar match path extension",
        )?;
        work.extend_existing_paths(
            &mut self.state.remaining,
            rejected_paths,
            "scalar match path extension",
        )?;
        self.state.values(with_true, work)
    }
}
impl<'a> Value<'a> {
    pub(super) fn expression(&self) -> &'a ResolvedExpr {
        &self.state.arms[self.state.index].value
    }
    pub(super) fn finish(
        mut self,
        program: &dyn ReplayContext,
        value_paths: Vec<ExprSkeletonPath>,
        work: &mut SkeletonWork<'_, '_>,
    ) -> Result<Next<'a>, Diagnostic> {
        let prefix = self
            .pending
            .next()
            .expect("scalar selected prefix retained");
        let sequenced = sequence_skeleton_paths(
            work.singleton_path(prefix, "scalar match selected prefix")?,
            &value_paths,
            work,
        )?;
        let finished = finish_conditional_result(
            program,
            work.function,
            self.state.expression,
            sequenced,
            work,
        )?;
        append_expr_paths(
            &mut self.state.results,
            finished,
            work,
            "scalar match arm result",
        )?;
        if !self.pending.as_slice().is_empty() {
            Ok(Next::Value(self))
        } else {
            self.state.index += 1;
            self.state.next(work)
        }
    }
}

#[cfg(test)]
mod tests;
