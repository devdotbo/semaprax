//! Ordinary construction ordinals order storage only; the memo checks Rc identity.
//! This keeps the legacy destructor and semantic observation ordering. New nodes
//! and every new tail/member carrier are paid on the existing replay ledger.
use super::*;
use std::rc::Rc;

impl SkeletonWork<'_, '_> {
    pub(super) fn ordinary_dag(
        &mut self,
        kind: ObservationDagKind,
    ) -> Result<Rc<ObservationDag>, Diagnostic> {
        self.charge(1, "ordinary DAG construction ordinal")?;
        let ordinal = self.next_ordinary_dag;
        let next = ordinal.checked_add(1).ok_or_else(|| {
            replay_error(self.function, "ordinary DAG construction ordinal overflowed")
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

    fn ordinary_freeze(
        &mut self,
        observations: &Observations,
    ) -> Result<Option<Rc<ObservationDag>>, Diagnostic> {
        if observations.items.is_empty() {
            return Ok(observations.prefix.clone());
        }
        self.budget.reserve_clone(
            self.function, &observations.items, "ordinary DAG sequence tail",
        )?;
        self.ordinary_dag(ObservationDagKind::Seq(
            observations.prefix.clone(), observations.items.clone(),
        )).map(Some)
    }

    pub(super) fn ordinary_union(
        &mut self,
        members: Vec<Observations>,
    ) -> Result<Observations, Diagnostic> {
        self.budget.reserve_array::<Rc<ObservationDag>>(
            self.function, members.len(), "ordinary DAG alternatives",
        )?;
        let mut nodes = Vec::with_capacity(members.len());
        for member in &members {
            let node = match self.ordinary_freeze(member)? {
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

    pub(super) fn ordinary_extend(
        &mut self,
        target: &mut Observations,
        suffix: &Observations,
    ) -> Result<(), Diagnostic> {
        let Some(prefix) = &suffix.prefix else {
            self.budget.reserve_vec(
                self.function, &mut target.items, suffix.items.len(), "ordinary flat suffix",
            )?;
            target.items.extend_from_slice(&suffix.items);
            return Ok(());
        };
        let joined = match self.ordinary_freeze(target)? {
            None => prefix.clone(),
            Some(head) => {
                self.budget.reserve_array::<Rc<ObservationDag>>(
                    self.function, 2, "ordinary DAG concatenation",
                )?;
                self.ordinary_dag(ObservationDagKind::Cat(vec![head, prefix.clone()]))?
            }
        };
        self.budget.reserve_clone(
            self.function, &suffix.items, "ordinary DAG suffix tail",
        )?;
        let items = suffix.items.clone();
        target.prefix = Some(joined);
        target.items = items;
        Ok(())
    }

    pub(super) fn ordinary_prepend(
        &mut self,
        target: &mut Observations,
        observation: Observation,
    ) -> Result<(), Diagnostic> {
        let prefix = target.prefix.as_ref().expect("caller observed prefix").clone();
        self.budget.reserve_array::<Observation>(
            self.function, 1, "ordinary DAG prepend head",
        )?;
        let head = self.ordinary_dag(ObservationDagKind::Seq(None, vec![observation]))?;
        self.budget.reserve_array::<Rc<ObservationDag>>(
            self.function, 2, "ordinary DAG prepend concatenation",
        )?;
        let joined = self.ordinary_dag(ObservationDagKind::Cat(vec![head, prefix]))?;
        target.prefix = Some(joined);
        Ok(())
    }
}

#[cfg(test)]
mod tests;
