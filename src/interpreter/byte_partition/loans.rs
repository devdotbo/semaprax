//! Closed ordinary receiver: one checked function, one invocation account, and
//! one owned runtime loan per plan ID. Copy descriptors never own finalization.
//! The eventual installer must bind each Owner to the plan's actual root and
//! drive `advance` from actual execution; this is not a source admission API.
use super::{authority::LoanCapacity, InvocationRuntime, Owner, Registry, View};
use crate::{
    construction_fusion::{domain::Refusal, partition::Preparation},
    loan_plan::LoanId,
};
use std::alloc::Layout;

#[derive(Clone, Copy)]
pub(crate) struct ViewId {
    invocation: u64,
    index: u16,
    generation: u64,
}
struct Slot {
    generation: u64,
    view: Option<View>,
}
pub(crate) struct Loans<'p, R: InvocationRuntime> {
    capacity: LoanCapacity<'p>,
    registry: Registry<R>,
    slots: Box<[Slot]>,
    point: u16,
}
impl<'p, R: InvocationRuntime> Loans<'p, R> {
    /// Header and complete fixed loan inventory are paid before allocation.
    /// Construction is inaccessible to source because LoanCapacity has no
    /// production constructor until the full ordinary factory is joined.
    pub(crate) fn new(capacity: LoanCapacity<'p>, mut runtime: R) -> Result<Self, Refusal> {
        if capacity.function().loan_plan.endpoints.is_empty() {
            return Err(Refusal::Identity);
        }
        runtime.reserve_metadata(Layout::new::<Self>())?;
        runtime.reserve_metadata(Layout::new::<Vec<Slot>>())?;
        runtime.reserve_metadata(
            Layout::array::<Slot>(capacity.slots()).map_err(|_| Refusal::Metadata)?,
        )?;
        let registry = Registry::new(runtime)?;
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(capacity.slots())
            .map_err(|_| Refusal::Metadata)?;
        assert_eq!(slots.capacity(), capacity.slots());
        slots.resize_with(capacity.slots(), || Slot {
            generation: 0,
            view: None,
        });
        Ok(Self {
            capacity,
            registry,
            slots: slots.into_boxed_slice(),
            point: 0,
        })
    }
    pub(crate) fn partition(
        &mut self,
        owner: R::Owner,
        count: u64,
        width: u64,
    ) -> Result<Vec<Owner>, (Preparation, R::Owner)> {
        self.registry
            .partition_with_capacity(owner, count, width, Some(&self.capacity))
    }
    /// Plan starts are checked against the current endpoint, not node-union
    /// kills/live-before. Alias descriptors copy ViewId; they never clone Loan.
    pub(crate) fn borrow(&mut self, owner: &Owner, id: LoanId) -> Result<ViewId, Refusal> {
        let result = (|| {
            self.registry.active()?;
            let plan = &self.capacity.function().loan_plan;
            let loan = plan
                .loans
                .get(id.0 as usize)
                .filter(|loan| loan.id == id)
                .ok_or(Refusal::Identity)?;
            let endpoint = plan
                .endpoints
                .get(self.point as usize)
                .ok_or(Refusal::Identity)?;
            if loan.start != endpoint.point || !endpoint.starts.contains(&id) {
                return Err(Refusal::Identity);
            }
            let slot = self.slots.get_mut(id.0 as usize).ok_or(Refusal::Identity)?;
            if slot.view.is_some() {
                return Err(Refusal::Borrowed);
            }
            let generation = slot.generation.checked_add(1).ok_or(Refusal::Exhausted)?;
            let view = self.registry.borrow(owner)?;
            slot.generation = generation;
            slot.view = Some(view);
            Ok(ViewId {
                invocation: self.registry.invocation,
                index: id.0,
                generation,
            })
        })();
        self.registry.select(result)
    }
    pub(crate) fn read<T>(&mut self, id: ViewId, f: impl FnOnce(&[u8]) -> T) -> Result<T, Refusal> {
        let result = (|| {
            self.registry.active()?;
            if id.invocation != self.registry.invocation {
                return Err(Refusal::Identity);
            }
            let slot = self.slots.get(id.index as usize).ok_or(Refusal::Identity)?;
            if slot.generation != id.generation {
                return Err(Refusal::Identity);
            }
            self.registry
                .read(slot.view.as_ref().ok_or(Refusal::Identity)?, f)
        })();
        self.registry.select(result)
    }
    /// The actual execution receiver supplies its next dense endpoint. Only an
    /// edge from the held current point can retire loans; an ambiguous edge is
    /// refused. All terminations are selected from exact end_edges.
    pub(crate) fn advance(&mut self, next: u16) -> Result<(), Refusal> {
        let result = (|| {
            self.registry.active()?;
            let plan = &self.capacity.function().loan_plan;
            let mut matches = plan
                .edges
                .iter()
                .enumerate()
                .filter(|(_, edge)| edge.from == self.point && edge.to == next);
            let (edge_index, _) = matches.next().ok_or(Refusal::Identity)?;
            if matches.next().is_some() {
                return Err(Refusal::Identity);
            }
            let edge_index = u16::try_from(edge_index).map_err(|_| Refusal::Identity)?;
            for loan in &plan.loans {
                if loan.end_edges.contains(&edge_index) {
                    if let Some(view) = self.slots[loan.id.0 as usize].view.as_mut() {
                        self.registry.end_view(view)?;
                        self.slots[loan.id.0 as usize].view = None;
                    }
                }
            }
            self.point = next;
            Ok(())
        })();
        self.registry.select(result)
    }
    pub(crate) fn store(
        &mut self,
        owner: &mut Owner,
        index: usize,
        bytes: &[u8],
    ) -> Result<(), Refusal> {
        self.registry.store(owner, index, bytes)
    }
    pub(crate) fn settle(&mut self, owner: &mut Owner) -> Result<(), Refusal> {
        self.registry.settle(owner)
    }
    /// Administrative scope teardown remains available after sticky refusal.
    /// It is never reported as canonical successful settlement/publication.
    pub(crate) fn abandon_views(&mut self) -> Result<(), Refusal> {
        let mut first = None;
        for slot in &mut self.slots {
            if let Some(mut view) = slot.view.take() {
                if let Err(error) = self.registry.end_view(&mut view) {
                    first.get_or_insert(error);
                }
            }
        }
        first.map_or(Ok(()), Err)
    }
}
impl<R: InvocationRuntime> Drop for Loans<'_, R> {
    fn drop(&mut self) {
        let _ = self.abandon_views();
    }
}

#[cfg(test)]
mod tests;
