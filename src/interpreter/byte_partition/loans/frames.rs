//! Invocation-held caller frames. Actual frame allocations debit the same
//! account on every entry; declaration count is not a runtime-call bound.
use super::{LoanCapacity, Loans, Slot, ViewId};
use crate::construction_fusion::domain::Refusal;
use crate::interpreter::byte_partition::InvocationRuntime;
use std::alloc::Layout;

pub(super) struct Frame<'p> {
    capacity: LoanCapacity<'p>,
    pub(super) slots: Box<[Slot]>,
    point: u16,
    nonce: u64,
    spare: Option<Box<[Slot]>>,
}
pub(super) fn lookup<'a>(
    id: ViewId,
    current: u64,
    slots: &'a [Slot],
    frames: &'a [Frame<'_>],
) -> Result<&'a Slot, Refusal> {
    let slots = if id.frame == current {
        slots
    } else {
        &frames
            .iter()
            .find(|frame| frame.nonce == id.frame)
            .ok_or(Refusal::Identity)?
            .slots
    };
    let slot = slots.get(id.index as usize).ok_or(Refusal::Identity)?;
    if slot.generation != id.generation || (slot.view.is_none() && slot.alias.is_none()) {
        return Err(Refusal::Identity);
    }
    Ok(slot)
}
/// Alias links always point strictly toward a held caller frame. The loop
/// exposes only the original table-owned physical loan, never a cloned token.
pub(super) fn resolve<'a>(
    mut id: ViewId,
    current: u64,
    slots: &'a [Slot],
    frames: &'a [Frame<'_>],
) -> Result<&'a super::View, Refusal> {
    for _ in 0..=frames.len() {
        let slot = lookup(id, current, slots, frames)?;
        if let Some(view) = &slot.view {
            return Ok(view);
        }
        id = slot.alias.ok_or(Refusal::Identity)?;
    }
    Err(Refusal::Identity)
}
impl<'p, R: InvocationRuntime> Loans<'p, R> {
    /// The actual argument binder must authenticate this parameter's borrowed
    /// root. This receiver only binds its local plan loan to a held caller ID.
    /// Caller loans keep their single finalizer; local aliases never end them.
    pub(crate) fn inherit_view(
        &mut self,
        id: crate::loan_plan::LoanId,
        caller: ViewId,
    ) -> Result<ViewId, Refusal> {
        let result = (|| {
            self.registry.active()?;
            if caller.invocation != self.registry.invocation
                || !self.frames.iter().any(|frame| frame.nonce == caller.frame)
            {
                return Err(Refusal::Identity);
            }
            resolve(caller, self.frame, &self.slots, &self.frames)?;
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
            if slot.view.is_some() || slot.alias.is_some() {
                return Err(Refusal::Borrowed);
            }
            let generation = slot.generation.checked_add(1).ok_or(Refusal::Exhausted)?;
            slot.generation = generation;
            slot.alias = Some(caller);
            Ok(ViewId {
                invocation: self.registry.invocation,
                frame: self.frame,
                index: id.0,
                generation,
            })
        })();
        self.registry.select(result)
    }

    /// This receiver requires separately authenticated call staging. Capacity
    /// authenticates the exact callee and program, not the actual call site.
    /// No evaluator entry installs this receiver before that proof is joined.
    pub(crate) fn enter_frame(&mut self, capacity: LoanCapacity<'p>) -> Result<(), Refusal> {
        let result = (|| {
            self.registry.active()?;
            if !std::ptr::eq(self.capacity.program(), capacity.program())
                || std::ptr::eq(self.capacity.function(), capacity.function())
                || self
                    .frames
                    .iter()
                    .any(|frame| std::ptr::eq(frame.capacity.function(), capacity.function()))
                || capacity.function().loan_plan.endpoints.is_empty()
            {
                return Err(Refusal::Identity);
            }
            let next_nonce = self.next_frame.checked_add(1).ok_or(Refusal::Exhausted)?;
            let replacement = if self.frames.len() == self.frames.capacity() {
                Some(
                    self.frames
                        .capacity()
                        .checked_mul(2)
                        .ok_or(Refusal::Metadata)?
                        .max(1),
                )
            } else {
                None
            };
            // Full replacement charge includes overlap with the held caller
            // directory. No refund follows a return or repeated invocation.
            if let Some(count) = replacement {
                self.registry
                    .reserve_consumer(Layout::new::<Vec<Frame<'p>>>())?;
                self.registry.reserve_consumer(
                    Layout::array::<Frame<'p>>(count).map_err(|_| Refusal::Metadata)?,
                )?;
            }
            // Allocate only actual local loan slots. A scalar helper has no
            // slot allocation; inherited descriptors still use caller slots.
            let count = capacity.function().loan_plan.loans.len();
            let reuse = self
                .spare
                .as_ref()
                .is_some_and(|slots| slots.len() >= count);
            if count != 0 && !reuse {
                self.registry.reserve_consumer(Layout::new::<Vec<Slot>>())?;
                self.registry.reserve_consumer(
                    Layout::array::<Slot>(count).map_err(|_| Refusal::Metadata)?,
                )?;
            }
            let slots = if count == 0 {
                Vec::new().into_boxed_slice()
            } else if reuse {
                self.spare.take().expect("checked reusable partition slots")
            } else {
                let mut slots = Vec::new();
                slots
                    .try_reserve_exact(count)
                    .expect("partition callee loan allocation invariant failure");
                assert_eq!(slots.capacity(), count);
                slots.resize_with(count, || Slot {
                    generation: 0,
                    view: None,
                    alias: None,
                });
                slots.into_boxed_slice()
            };
            if let Some(count) = replacement {
                let mut directory = Vec::new();
                directory
                    .try_reserve_exact(count)
                    .expect("partition frame directory allocation invariant failure");
                assert_eq!(directory.capacity(), count);
                directory.append(&mut self.frames);
                self.frames = directory;
            }
            let caller = Frame {
                capacity: std::mem::replace(&mut self.capacity, capacity),
                slots: std::mem::replace(&mut self.slots, slots),
                point: self.point,
                nonce: self.frame,
                spare: self.spare.take(),
            };
            self.frames.push(caller);
            self.frame = self.next_frame;
            self.next_frame = next_nonce;
            self.point = 0;
            Ok(())
        })();
        self.registry.select(result)
    }
    /// Only this frame's physical loans are ended. Borrowed caller descriptors
    /// remain table-owned in saved frames, and children remain registry-owned.
    /// Administrative teardown does not certify a canonical return action.
    pub(crate) fn leave_frame(&mut self) -> Result<(), Refusal> {
        if self.frames.is_empty() {
            return self.registry.select(Err(Refusal::Identity));
        }
        let mut first = None;
        for slot in &mut self.slots {
            slot.alias = None;
            if let Some(mut view) = slot.view.take() {
                if let Err(error) = self.registry.end_view(&mut view) {
                    first.get_or_insert(error);
                }
            }
        }
        let caller = self.frames.pop().expect("checked partition caller frame");
        self.capacity = caller.capacity;
        let retired = std::mem::replace(&mut self.slots, caller.slots);
        // Retain the largest paid idle array. Its old view descriptors remain
        // invalid because each entry receives a fresh nonreused frame nonce.
        let mut spare = self.spare.take();
        for candidate in [caller.spare, Some(retired)].into_iter().flatten() {
            if spare
                .as_ref()
                .is_none_or(|current| candidate.len() > current.len())
            {
                spare = Some(candidate);
            }
        }
        self.spare = spare;
        self.point = caller.point;
        self.frame = caller.nonce;
        self.registry.select(first.map_or(Ok(()), Err))
    }
}

#[cfg(test)]
mod tests;
