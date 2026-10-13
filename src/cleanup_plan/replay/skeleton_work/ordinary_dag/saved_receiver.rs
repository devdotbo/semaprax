//! Complete independent original node/kind/destructor and ordinary constructors.
//! The pre-frontier physical receiver remains executable against these types.
use super::*;

mod dag_storage {
    pub(super) struct DestructionScratch {
        pub(super) pending: Vec<std::rc::Rc<super::ObservationDag>>,
        pub(super) registered_edges: usize,
        pub(super) next_ordinal: usize,
    }
}

pub(super) struct SkeletonWork<'a, 'b> {
    pub(super) function: &'a ResolvedFunction,
    pub(super) budget: &'b mut ReplayBudget,
    pub(super) next_ordinary_dag: usize,
}
impl<'a, 'b> SkeletonWork<'a, 'b> {
    pub(super) fn new(
        program: &'a dyn crate::cleanup::ReplayContext,
        function: &'a ResolvedFunction,
        budget: &'b mut ReplayBudget,
    ) -> Self {
        // Complete entry inventories are still initialized outside the measured
        // receiver, just as in the original owning test. No DAG is produced here.
        let initialized = super::super::SkeletonWork::new(program, function, budget);
        drop(initialized);
        Self {
            function,
            budget,
            next_ordinary_dag: 0,
        }
    }
    fn charge(&mut self, units: usize, phase: &str) -> Result<(), Diagnostic> {
        self.budget.charge_skeleton(self.function, units, phase)
    }
}

#[derive(Clone, Default)]
pub(super) struct Observations {
    pub(super) prefix: Option<std::rc::Rc<ObservationDag>>,
    pub(super) items: Vec<Observation>,
}

/// A set of observation sequences. `Seq` is `prefix` then `items`; `Cat`
/// concatenates its parts in order; `Alt` is the union of its members.
pub(super) struct ObservationDag {
    pub(super) ordinal: Option<usize>,
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
            ordinal: None,
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

    #[cfg(test)]
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
    pub(super) fn ordinary_dag(
        &mut self,
        kind: ObservationDagKind,
    ) -> Result<Rc<ObservationDag>, Diagnostic> {
        self.charge(1, "ordinary DAG construction ordinal")?;
        let ordinal = self.next_ordinary_dag;
        let next = ordinal.checked_add(1).ok_or_else(|| {
            replay_error(
                self.function,
                "ordinary DAG construction ordinal overflowed",
            )
        })?;
        self.budget.reserve_bytes(
            self.function,
            physical_clone::rc_backing_bytes::<ObservationDag>(),
            "ordinary observation DAG node",
        )?;
        let mut node = ObservationDag::legacy(kind);
        node.ordinal = Some(ordinal);
        let node = Rc::new(node);
        self.next_ordinary_dag = next;
        Ok(node)
    }

    #[cfg(test)]
    fn ordinary_freeze(
        &mut self,
        observations: &Observations,
    ) -> Result<Option<Rc<ObservationDag>>, Diagnostic> {
        if observations.items.is_empty() {
            return Ok(observations.prefix.clone());
        }
        self.budget.reserve_clone(
            self.function,
            &observations.items,
            "ordinary DAG sequence tail",
        )?;
        self.ordinary_dag(ObservationDagKind::Seq(
            observations.prefix.clone(),
            observations.items.clone(),
        ))
        .map(Some)
    }

    /// Owned union members already paid for their tail backing.
    fn ordinary_freeze_owned(
        &mut self,
        observations: Observations,
    ) -> Result<Option<Rc<ObservationDag>>, Diagnostic> {
        if observations.items.is_empty() {
            return Ok(observations.prefix);
        }
        self.ordinary_dag(ObservationDagKind::Seq(
            observations.prefix,
            observations.items,
        ))
        .map(Some)
    }

    pub(super) fn ordinary_union(
        &mut self,
        members: Vec<Observations>,
    ) -> Result<Observations, Diagnostic> {
        self.budget.reserve_array::<Rc<ObservationDag>>(
            self.function,
            members.len(),
            "ordinary DAG alternatives",
        )?;
        let mut nodes = Vec::with_capacity(members.len());
        for member in members {
            let node = match self.ordinary_freeze_owned(member)? {
                Some(node) => node,
                None => self.ordinary_dag(ObservationDagKind::Seq(None, Vec::new()))?,
            };
            nodes.push(node);
        }
        Ok(Observations {
            prefix: Some(self.ordinary_dag(ObservationDagKind::Alt(nodes))?),
            items: Vec::new(),
        })
    }

    #[cfg(test)]
    pub(super) fn ordinary_extend(
        &mut self,
        target: &mut Observations,
        suffix: &Observations,
    ) -> Result<(), Diagnostic> {
        let Some(prefix) = &suffix.prefix else {
            self.budget.reserve_vec(
                self.function,
                &mut target.items,
                suffix.items.len(),
                "ordinary flat suffix",
            )?;
            target.items.extend_from_slice(&suffix.items);
            return Ok(());
        };
        let joined = match self.ordinary_freeze(target)? {
            None => prefix.clone(),
            Some(head) => {
                self.budget.reserve_array::<Rc<ObservationDag>>(
                    self.function,
                    2,
                    "ordinary DAG concatenation",
                )?;
                self.ordinary_dag(ObservationDagKind::Cat(vec![head, prefix.clone()]))?
            }
        };
        self.budget
            .reserve_clone(self.function, &suffix.items, "ordinary DAG suffix tail")?;
        let items = suffix.items.clone();
        target.prefix = Some(joined);
        target.items = items;
        Ok(())
    }

    pub(super) fn ordinary_extend_owned(
        &mut self,
        mut target: Observations,
        suffix: &Observations,
    ) -> Result<Observations, Diagnostic> {
        let Some(prefix) = &suffix.prefix else {
            self.budget.reserve_vec(
                self.function,
                &mut target.items,
                suffix.items.len(),
                "ordinary flat suffix",
            )?;
            target.items.extend_from_slice(&suffix.items);
            return Ok(target);
        };
        let joined = match self.ordinary_freeze_owned(target)? {
            None => prefix.clone(),
            Some(head) => {
                self.budget.reserve_array::<Rc<ObservationDag>>(
                    self.function,
                    2,
                    "ordinary DAG concatenation",
                )?;
                self.ordinary_dag(ObservationDagKind::Cat(vec![head, prefix.clone()]))?
            }
        };
        self.budget
            .reserve_clone(self.function, &suffix.items, "ordinary DAG suffix tail")?;
        Ok(Observations {
            prefix: Some(joined),
            items: suffix.items.clone(),
        })
    }

    pub(super) fn ordinary_prepend(
        &mut self,
        target: &mut Observations,
        observation: Observation,
    ) -> Result<(), Diagnostic> {
        let prefix = target
            .prefix
            .as_ref()
            .expect("caller observed prefix")
            .clone();
        self.budget
            .reserve_array::<Observation>(self.function, 1, "ordinary DAG prepend head")?;
        let head = self.ordinary_dag(ObservationDagKind::Seq(None, vec![observation]))?;
        self.budget.reserve_array::<Rc<ObservationDag>>(
            self.function,
            2,
            "ordinary DAG prepend concatenation",
        )?;
        let joined = self.ordinary_dag(ObservationDagKind::Cat(vec![head, prefix]))?;
        target.prefix = Some(joined);
        Ok(())
    }
}

// End of byte-locked original ordinary constructor body.

// Test-only lexical access bridges; the complete saved receiver above is verbatim.
impl Observations {
    pub(super) fn union_for_test(members: Vec<Observations>) -> Self {
        Self::union(members)
    }
}

impl SkeletonWork<'_, '_> {
    pub(super) fn charge_for_test(&mut self, units: usize, phase: &str) -> Result<(), Diagnostic> {
        self.charge(units, phase)
    }
}
