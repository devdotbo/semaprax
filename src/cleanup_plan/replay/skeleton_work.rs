//! Budget-charged skeleton path materialization for the typed-HIR replay.
//!
//! Path observations are shared behind `Rc`, so sequencing a prefix with each
//! suffix copies pointers rather than deep-copying every identity and place.
//! Every charge and materialization count is unchanged.
use super::*;
mod dag_storage;

pub(super) struct SkeletonWork<'a, 'b> {
    pub(super) function: &'a ResolvedFunction,
    pub(super) budget: &'b mut ReplayBudget,
    pub(super) renewal_bindings: crate::cleanup_plan::renewal_profile::RenewalBindings<'a>,
    dag_scratch: Option<std::rc::Rc<std::cell::RefCell<dag_storage::DestructionScratch>>>,
    held_renewals: Option<&'a crate::cleanup_plan::renewal_profile::RenewalBindings<'a>>,
    /// Authored same-owner append and direct literal-format String moves.
    pub(super) string_owner_moves: BTreeSet<ExpressionId>,
    pub(super) string_condition_reads: BTreeSet<ExpressionId>,
    /// Authenticated execution identity of a lexical-profile function.
    pub(super) owner: Option<crate::hir::FunctionExecutionId>,
}

impl<'a, 'b> SkeletonWork<'a, 'b> {
    #[cfg(test)]
    pub(super) fn new(
        program: &'a dyn crate::cleanup::ReplayContext,
        function: &'a ResolvedFunction,
        budget: &'b mut ReplayBudget,
    ) -> Self {
        Self::with_index(program, function, budget, None)
    }
    pub(super) fn with_index(
        program: &'a dyn crate::cleanup::ReplayContext,
        function: &'a ResolvedFunction,
        budget: &'b mut ReplayBudget,
        index: Option<&'a crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    ) -> Self {
        Self {
            function,
            budget,
            dag_scratch: None,
            held_renewals: None,
            renewal_bindings: crate::cleanup_plan::renewal_profile::bindings_with_index(
                program, function, index,
            ),
            string_owner_moves: crate::literal_format::moving_string_operands(function),
            string_condition_reads: crate::string_ops::conditions::function_reads(function),
            owner: if super::lexical_cert::requires_lexical(function) {
                crate::hir::lexical_exit::owner(program, function).ok()
            } else {
                None
            },
        }
    }

    pub(super) fn checked_with_index(
        program: &'a dyn crate::cleanup::ReplayContext,
        function: &'a ResolvedFunction,
        budget: &'b mut ReplayBudget,
        index: Option<&'a crate::loop_calls::byte_writer::ModuleIndex<'a>>,
    ) -> Result<Self, Diagnostic> {
        if budget.physical.is_none() {
            return Ok(Self::with_index(program, function, budget, index));
        }
        selected_profile::validate(program, function, budget)?;
        let owner = selected_owner::owner(program, function, budget)?;
        let held_renewals = program.primitive_renewals(function).ok_or_else(|| {
            replay_error(
                function,
                "selected skeleton requires its independently rebuilt phase site inventory",
            )
        })?;
        // The selected entry independently excludes literal formatting and
        // owned String condition-place reads before this constructor. Those
        // specialized legacy indexes must not be constructed then discarded.
        Ok(Self {
            function,
            budget,
            renewal_bindings: crate::cleanup_plan::renewal_profile::RenewalBindings::empty(),
            dag_scratch: None,
            held_renewals: Some(held_renewals),
            string_owner_moves: BTreeSet::new(),
            string_condition_reads: BTreeSet::new(),
            owner: Some(owner),
        })
    }

    pub(super) fn renewal_binding(
        &self,
        at: &crate::hir::ExpressionId,
    ) -> Option<&'a crate::hir::ResolvedBinding> {
        match self.held_renewals {
            Some(sites) => sites.get(at),
            None => self.renewal_bindings.get(at),
        }
    }
}

impl SkeletonWork<'_, '_> {
    pub(super) fn charge(&mut self, units: usize, phase: &str) -> Result<(), Diagnostic> {
        self.budget.charge_skeleton(self.function, units, phase)
    }

    /// Preserve legacy sequencing/work counters for a previously uncharged
    /// move, while reserving selected vector growth before that move occurs.
    pub(super) fn push_existing_path(
        &mut self,
        target: &mut Vec<ExprSkeletonPath>,
        path: ExprSkeletonPath,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.budget.reserve_vec(self.function, target, 1, phase)?;
        target.push(path);
        Ok(())
    }
    pub(super) fn extend_existing_paths(
        &mut self,
        target: &mut Vec<ExprSkeletonPath>,
        paths: Vec<ExprSkeletonPath>,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.budget
            .reserve_vec(self.function, target, paths.len(), phase)?;
        target.extend(paths);
        Ok(())
    }

    pub(super) fn clone_owned<T: Clone + ClonePayload>(
        &mut self,
        value: &T,
        phase: &str,
    ) -> Result<T, Diagnostic> {
        self.charge(1, phase)?;
        self.budget.reserve_clone(self.function, value, phase)?;
        note_skeleton_materialization();
        Ok(value.clone())
    }

    /// Meter an existing uncharged clone without changing its legacy work count.
    pub(super) fn clone_payload<T: Clone + ClonePayload>(
        &mut self,
        value: &T,
        phase: &str,
    ) -> Result<T, Diagnostic> {
        self.budget.reserve_clone(self.function, value, phase)?;
        Ok(value.clone())
    }

    pub(super) fn push_expr_path(
        &mut self,
        paths: &mut Vec<ExprSkeletonPath>,
        path: ExprSkeletonPath,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        self.budget.reserve_vec(self.function, paths, 1, phase)?;
        paths.push(path);
        Ok(())
    }

    pub(super) fn push_skeleton_path(
        &mut self,
        paths: &mut Vec<SkeletonPath>,
        path: SkeletonPath,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        self.budget.reserve_vec(self.function, paths, 1, phase)?;
        paths.push(path);
        Ok(())
    }

    pub(super) fn singleton_path(
        &mut self,
        path: ExprSkeletonPath,
        phase: &str,
    ) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
        let mut paths = Vec::new();
        self.push_expr_path(&mut paths, path, phase)?;
        Ok(paths)
    }

    pub(super) fn clone_expr_path(
        &mut self,
        path: &ExprSkeletonPath,
        phase: &str,
    ) -> Result<ExprSkeletonPath, Diagnostic> {
        self.charge(1, phase)?;
        self.budget.reserve_clone(self.function, path, phase)?;
        note_skeleton_materialization();
        Ok(path.clone())
    }

    pub(super) fn clone_observations(
        &mut self,
        observations: &Observations,
        phase: &str,
    ) -> Result<Observations, Diagnostic> {
        self.charge(1, phase)?;
        self.budget
            .reserve_clone(self.function, observations, phase)?;
        note_skeleton_materialization();
        Ok(observations.clone())
    }

    pub(super) fn extend_observations(
        &mut self,
        target: &mut Observations,
        observations: &Observations,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        if self.budget.physical.is_some() {
            if target.prefix.is_some() || observations.prefix.is_some() {
                if !self.budget.merge_paths {
                    return Err(replay_error(
                        self.function,
                        "selected lexical replay requires flat observations",
                    ));
                }
                return self.extend_factored_observations(target, observations);
            }
            self.budget.reserve_vec(
                self.function,
                &mut target.items,
                observations.items.len(),
                phase,
            )?;
        }
        target.extend(observations);
        Ok(())
    }

    pub(super) fn push_observation(
        &mut self,
        path: &mut ExprSkeletonPath,
        observation: SkeletonObservation,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.charge(1, phase)?;
        note_skeleton_materialization();
        if self.budget.physical.is_some()
            && path.observations.prefix.is_some()
            && !self.budget.merge_paths
        {
            return Err(replay_error(
                self.function,
                "selected lexical replay requires flat observations",
            ));
        }
        self.budget
            .reserve_vec(self.function, &mut path.observations.items, 1, phase)?;
        self.budget
            .reserve_bytes(self.function, physical_clone::observation_rc_bytes(), phase)?;
        let observation = std::rc::Rc::new(observation);
        path.continuation.record(&observation);
        path.observations.push(observation);
        Ok(())
    }
    pub(super) fn insert_front_observation(
        &mut self,
        observations: &mut Observations,
        observation: SkeletonObservation,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        self.budget
            .reserve_bytes(self.function, physical_clone::observation_rc_bytes(), phase)?;
        if self.budget.physical.is_some() && observations.prefix.is_some() {
            if !self.budget.merge_paths {
                return Err(replay_error(
                    self.function,
                    "selected lexical replay requires flat observations",
                ));
            }
            return self.prepend_factored_observation(observations, observation.into());
        }
        self.budget
            .reserve_vec(self.function, &mut observations.items, 1, phase)?;
        observations.insert_front(observation.into());
        Ok(())
    }
}

pub(super) fn empty_expr_path() -> ExprSkeletonPath {
    ExprSkeletonPath {
        observations: Observations::default(),
        owned_source: None,
        failed: false,
        residual: false,
        continuation: lexical_skeleton::Continuation::Normal,
    }
}

type Observation = std::rc::Rc<SkeletonObservation>;

/// The observation sequences of one skeleton path.
///
/// The enumerating replay keeps every path flat: `prefix` stays `None` and
/// `items` is exactly the former observation vector, with the same clone,
/// append and charge behavior. Only the factored replay merges paths that
/// carry identical cleanup state; their sequences then become one shared
/// `ObservationDag` prefix denoting the set of all merged sequences, followed
/// by the flat `items` appended afterwards.
#[derive(Clone, Default)]
pub(super) struct Observations {
    pub(super) prefix: Option<std::rc::Rc<ObservationDag>>,
    pub(super) items: Vec<Observation>,
}

/// A set of observation sequences. `Seq` is `prefix` then `items`; `Cat`
/// concatenates its parts in order; `Alt` is the union of its members.
pub(super) struct ObservationDag {
    pub(super) kind: ObservationDagKind,
    scratch: Option<std::rc::Rc<std::cell::RefCell<dag_storage::DestructionScratch>>>,
}

pub(super) enum ObservationDagKind {
    Seq(Option<std::rc::Rc<ObservationDag>>, Vec<Observation>),
    Cat(Vec<std::rc::Rc<ObservationDag>>),
    Alt(Vec<std::rc::Rc<ObservationDag>>),
}

impl Drop for ObservationDag {
    /// Selected DAG nodes share a precharged destruction frontier. No backing
    /// allocation occurs during their destruction, including deeply shared DAGs.
    fn drop(&mut self) {
        if !self.has_children() {
            return;
        }
        if let Some(scratch) = self.scratch.clone() {
            let mut scratch = scratch.borrow_mut();
            self.drain_children(&mut scratch.pending);
            while let Some(child) = scratch.pending.pop() {
                if let Ok(mut node) = std::rc::Rc::try_unwrap(child) {
                    node.drain_children(&mut scratch.pending);
                    // `node` now has no children, so its Drop never reborrows.
                }
            }
            return;
        }
        // Preserve the ordinary factored route's destructor behavior.
        let mut pending = self.take_children();
        while let Some(child) = pending.pop() {
            if let Ok(mut node) = std::rc::Rc::try_unwrap(child) {
                pending.extend(node.take_children());
            }
        }
    }
}

impl ObservationDag {
    pub(super) fn legacy(kind: ObservationDagKind) -> Self {
        Self {
            kind,
            scratch: None,
        }
    }

    fn has_children(&self) -> bool {
        match &self.kind {
            ObservationDagKind::Seq(prefix, _) => prefix.is_some(),
            ObservationDagKind::Cat(parts) | ObservationDagKind::Alt(parts) => !parts.is_empty(),
        }
    }

    fn drain_children(&mut self, pending: &mut Vec<std::rc::Rc<Self>>) {
        match &mut self.kind {
            ObservationDagKind::Seq(prefix, _) => {
                if let Some(child) = prefix.take() {
                    pending.push(child);
                }
            }
            ObservationDagKind::Cat(parts) | ObservationDagKind::Alt(parts) => {
                pending.append(parts);
            }
        }
    }

    fn take_children(&mut self) -> Vec<std::rc::Rc<Self>> {
        match &mut self.kind {
            ObservationDagKind::Seq(prefix, _) => prefix.take().into_iter().collect(),
            ObservationDagKind::Cat(parts) | ObservationDagKind::Alt(parts) => {
                std::mem::take(parts)
            }
        }
    }
}

impl Observations {
    pub(super) fn push(&mut self, observation: Observation) {
        self.items.push(observation);
    }

    pub(super) fn insert_front(&mut self, observation: Observation) {
        match self.prefix.take() {
            None => self.items.insert(0, observation),
            Some(prefix) => {
                let head = std::rc::Rc::new(ObservationDag::legacy(ObservationDagKind::Seq(
                    None,
                    vec![observation],
                )));
                self.prefix = Some(std::rc::Rc::new(ObservationDag::legacy(
                    ObservationDagKind::Cat(vec![head, prefix]),
                )));
            }
        }
    }

    /// Append every sequence of `suffix` to every sequence of `self`.
    pub(super) fn extend(&mut self, suffix: &Observations) {
        let Some(suffix_prefix) = &suffix.prefix else {
            self.items.extend_from_slice(&suffix.items);
            return;
        };
        self.prefix = Some(match self.freeze() {
            None => suffix_prefix.clone(),
            Some(head) => std::rc::Rc::new(ObservationDag::legacy(ObservationDagKind::Cat(vec![
                head,
                suffix_prefix.clone(),
            ]))),
        });
        self.items = suffix.items.clone();
    }

    /// The whole sequence set as one DAG node, or `None` for the empty sequence.
    pub(super) fn freeze(&self) -> Option<std::rc::Rc<ObservationDag>> {
        if self.items.is_empty() {
            return self.prefix.clone();
        }
        Some(std::rc::Rc::new(ObservationDag::legacy(
            ObservationDagKind::Seq(self.prefix.clone(), self.items.clone()),
        )))
    }

    fn union(members: Vec<Observations>) -> Self {
        let members = members
            .iter()
            .map(|member| {
                member.freeze().unwrap_or_else(|| {
                    std::rc::Rc::new(ObservationDag::legacy(ObservationDagKind::Seq(
                        None,
                        Vec::new(),
                    )))
                })
            })
            .collect();
        Self {
            prefix: Some(std::rc::Rc::new(ObservationDag::legacy(
                ObservationDagKind::Alt(members),
            ))),
            items: Vec::new(),
        }
    }

    pub(super) fn parts(&self) -> (Option<&std::rc::Rc<ObservationDag>>, &[Observation]) {
        (self.prefix.as_ref(), &self.items)
    }

    fn identity(&self) -> (Option<usize>, &[Observation]) {
        (
            self.prefix
                .as_ref()
                .map(|prefix| std::rc::Rc::as_ptr(prefix) as usize),
            &self.items,
        )
    }
}

impl From<Vec<Observation>> for Observations {
    fn from(items: Vec<Observation>) -> Self {
        Self {
            prefix: None,
            items,
        }
    }
}

/// The flat tail. It is the complete sequence on every enumerating path,
/// where nothing is merged.
impl std::ops::Deref for Observations {
    type Target = [Observation];

    fn deref(&self) -> &Self::Target {
        &self.items
    }
}

impl std::fmt::Debug for Observations {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.prefix.is_some() {
            formatter.write_str("<merged> ")?;
        }
        formatter.debug_list().entries(&self.items).finish()
    }
}

/// Flat sequences compare by content; the enumerating comparison sorts only
/// flat paths. A merged prefix compares by identity.
impl PartialEq for Observations {
    fn eq(&self, other: &Self) -> bool {
        self.identity() == other.identity()
    }
}

impl Eq for Observations {}

impl PartialOrd for Observations {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Observations {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.identity().cmp(&other.identity())
    }
}

impl SkeletonWork<'_, '_> {
    /// Factored replay only: merge paths whose cleanup state is identical
    /// into one path carrying the union of their observation sequences. Every
    /// later skeleton step depends only on that state, so the union denotes
    /// exactly the paths enumeration would produce.
    pub(super) fn merged_paths(
        &mut self,
        paths: Vec<ExprSkeletonPath>,
    ) -> Result<Vec<ExprSkeletonPath>, Diagnostic> {
        if !self.budget.merge_paths || paths.len() < 2 {
            return Ok(paths);
        }
        self.charge(paths.len(), "factored path-state merge")?;
        type Key = (
            Option<CleanupPlace>,
            bool,
            bool,
            lexical_skeleton::Continuation,
        );
        type Group = (ExprSkeletonPath, Vec<Observations>);
        let mut order = BTreeMap::<Key, usize>::new();
        let mut groups = Vec::<Group>::new();
        for path in paths {
            let source = self.clone_payload(&path.owned_source, "factored merge source key")?;
            let key = (
                source,
                path.failed,
                path.residual,
                path.continuation.clone(),
            );
            match order.get(&key).copied() {
                Some(index) => {
                    self.budget.reserve_vec(
                        self.function,
                        &mut groups[index].1,
                        1,
                        "factored merge members",
                    )?;
                    groups[index].1.push(path.observations);
                }
                None => {
                    self.budget.reserve_btree_node::<Key, usize>(
                        self.function,
                        "factored merge identity index",
                    )?;
                    self.budget.reserve_vec(
                        self.function,
                        &mut groups,
                        1,
                        "factored merge groups",
                    )?;
                    self.budget.reserve_array::<Observations>(
                        self.function,
                        1,
                        "factored first merge member",
                    )?;
                    let observations =
                        self.clone_payload(&path.observations, "factored first member tail")?;
                    order.insert(key, groups.len());
                    groups.push((path, vec![observations]));
                }
            }
        }
        self.budget.reserve_array::<ExprSkeletonPath>(
            self.function,
            groups.len(),
            "factored merged paths",
        )?;
        let mut result = Vec::with_capacity(groups.len());
        for (mut path, members) in groups {
            if members.len() > 1 {
                path.observations = if self.budget.physical.is_some() {
                    self.union_observations(members)?
                } else {
                    Observations::union(members)
                };
            }
            result.push(path);
        }
        Ok(result)
    }

    pub(super) fn merged_suffixes<'s>(
        &mut self,
        paths: &'s [ExprSkeletonPath],
    ) -> Result<std::borrow::Cow<'s, [ExprSkeletonPath]>, Diagnostic> {
        if !self.budget.merge_paths || paths.len() < 2 {
            return Ok(std::borrow::Cow::Borrowed(paths));
        }
        self.budget.reserve_array::<ExprSkeletonPath>(
            self.function,
            paths.len(),
            "factored suffix path copies",
        )?;
        let mut owned = Vec::with_capacity(paths.len());
        for path in paths {
            owned.push(self.clone_payload(path, "factored suffix path backing")?);
        }
        Ok(std::borrow::Cow::Owned(self.merged_paths(owned)?))
    }

    /// Call states additionally merge only with identical commit lists.
    pub(super) fn merged_call_states(
        &mut self,
        states: Vec<CallSkeletonState>,
    ) -> Result<Vec<CallSkeletonState>, Diagnostic> {
        if !self.budget.merge_paths || states.len() < 2 {
            return Ok(states);
        }
        self.charge(states.len(), "factored call-state merge")?;
        type Key = (
            Option<CleanupPlace>,
            bool,
            bool,
            lexical_skeleton::Continuation,
            Vec<(u32, CleanupPlace)>,
        );
        type Group = (CallSkeletonState, Vec<Observations>);
        let mut order = BTreeMap::<Key, usize>::new();
        let mut groups = Vec::<Group>::new();
        for (path, commits) in states {
            let source = self.clone_payload(&path.owned_source, "factored call source key")?;
            let key_commits = self.clone_payload(&commits, "factored call commit key")?;
            let key = (
                source,
                path.failed,
                path.residual,
                path.continuation.clone(),
                key_commits,
            );
            match order.get(&key).copied() {
                Some(index) => {
                    self.budget.reserve_vec(
                        self.function,
                        &mut groups[index].1,
                        1,
                        "factored call merge members",
                    )?;
                    groups[index].1.push(path.observations);
                }
                None => {
                    self.budget.reserve_btree_node::<Key, usize>(
                        self.function,
                        "factored call identity index",
                    )?;
                    self.budget.reserve_vec(
                        self.function,
                        &mut groups,
                        1,
                        "factored call merge groups",
                    )?;
                    self.budget.reserve_array::<Observations>(
                        self.function,
                        1,
                        "factored first call member",
                    )?;
                    let observations =
                        self.clone_payload(&path.observations, "factored first call tail")?;
                    order.insert(key, groups.len());
                    groups.push(((path, commits), vec![observations]));
                }
            }
        }
        self.budget.reserve_array::<CallSkeletonState>(
            self.function,
            groups.len(),
            "factored merged call states",
        )?;
        let mut result = Vec::with_capacity(groups.len());
        for ((mut path, commits), members) in groups {
            if members.len() > 1 {
                path.observations = if self.budget.physical.is_some() {
                    self.union_observations(members)?
                } else {
                    Observations::union(members)
                };
            }
            result.push((path, commits));
        }
        Ok(result)
    }
}
