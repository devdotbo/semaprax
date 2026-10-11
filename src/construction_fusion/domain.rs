//! Bounded executor-neutral reference protocol. No source/API activation.
//! Backing allocation is an explicit callback at the first original Bytes site;
//! physical OOM remains the callback's invariant fail-stop, not a new status.
use super::Descriptor;
pub(crate) const MAX_VIEWS: usize = 64;
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Phase {
    Unstarted,
    Building,
    Complete,
    Aborted,
    Settled,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Refusal {
    Metadata,
    Phase,
    Identity,
    Range,
    Borrowed,
    Exhausted,
    Unpublished,
}
pub(crate) struct ModuleIdentity {
    _unique_address: u64,
}
impl ModuleIdentity {
    pub(crate) fn new() -> Self {
        Self { _unique_address: 0 }
    }
}
/// Held host owner; fields are private, and no reset or wrap operation exists.
pub(crate) struct Issuer {
    next: u64,
}
impl Issuer {
    pub(crate) fn new() -> Self {
        Self { next: 1 }
    }
    fn mint(&mut self) -> Result<u64, Refusal> {
        let value = self.next;
        self.next = self.next.checked_add(1).ok_or(Refusal::Exhausted)?;
        Ok(value)
    }
}
#[derive(Clone, Copy)]
struct Slot {
    nonce: u64,
    generation: u64,
    live: bool,
    published: bool,
    borrows: usize,
}
#[derive(Clone, Copy)]
struct View {
    nonce: u64,
    slot: usize,
    generation: u64,
    active: bool,
}
/// Not Clone/Copy: successful move/drop invalidates the prior generation.
#[must_use]
pub(crate) struct Lease<'a> {
    lifetime: std::marker::PhantomData<&'a Issuer>,
    issuer: usize,
    domain: u64,
    module: usize,
    slot: usize,
    nonce: u64,
    generation: u64,
    offset: usize,
    len: usize,
}
#[must_use]
pub(crate) struct Borrow<'a> {
    lifetime: std::marker::PhantomData<&'a Issuer>,
    issuer: usize,
    domain: u64,
    module: usize,
    view: usize,
    nonce: u64,
    slot: usize,
    generation: u64,
    offset: usize,
    len: usize,
}
pub(crate) trait Backing {
    type Owner;
    fn allocate(&mut self, extent: usize) -> Self::Owner;
    fn release(&mut self, owner: Self::Owner);
}
#[must_use]
pub(crate) struct Domain<'a, B: Backing> {
    descriptor: &'a Descriptor<'a>,
    issuer: &'a mut Issuer,
    module: &'a ModuleIdentity,
    domain: u64,
    phase: Phase,
    slots: Vec<Slot>,
    views: Vec<View>,
    issued: usize,
    live: usize,
    construction_pin: bool,
    backing: Option<std::mem::ManuallyDrop<B::Owner>>,
    executor: B,
}
impl<'a, B: Backing> Domain<'a, B> {
    pub(crate) fn metadata_bytes(count: usize) -> Option<usize> {
        std::mem::size_of::<Self>()
            .checked_add(count.checked_mul(std::mem::size_of::<Slot>())?)?
            .checked_add(MAX_VIEWS.checked_mul(std::mem::size_of::<View>())?)
    }
    pub(crate) fn new(
        descriptor: &'a Descriptor<'a>,
        issuer: &'a mut Issuer,
        module: &'a ModuleIdentity,
        executor: B,
        metadata_limit: usize,
    ) -> Result<Self, Refusal> {
        let count = descriptor.lease_slots();
        let required = Self::metadata_bytes(count).ok_or(Refusal::Metadata)?;
        if required > metadata_limit {
            return Err(Refusal::Metadata);
        }
        // Preflight reserves logical capacity only. No physical metadata or
        // payload allocator runs before the first original Bytes operand.
        let slots = Vec::new();
        let views = Vec::new();
        let domain = issuer.mint()?;
        Ok(Self {
            descriptor,
            issuer,
            module,
            domain,
            phase: Phase::Unstarted,
            slots,
            views,
            issued: 0,
            live: 0,
            construction_pin: false,
            backing: None,
            executor,
        })
    }
    fn issuer_key(&self) -> usize {
        std::ptr::from_ref(&*self.issuer) as usize
    }
    fn module_key(&self) -> usize {
        std::ptr::from_ref(self.module) as usize
    }
    pub(crate) fn phase(&self) -> Phase {
        self.phase
    }
    pub(crate) fn start(&mut self) -> Result<(), Refusal> {
        if self.phase != Phase::Unstarted {
            return Err(Refusal::Phase);
        }
        self.phase = Phase::Building;
        self.construction_pin = true;
        Ok(())
    }
    /// The caller invokes this only at the authenticated original byte operand,
    /// after the row's earlier String operand has completed. Nothing at start,
    /// Vec allocation or metadata construction calls the payload allocator.
    pub(crate) fn issue(&mut self, ordinal: usize) -> Result<Lease<'a>, Refusal> {
        if self.phase != Phase::Building {
            return Err(Refusal::Phase);
        }
        if ordinal != self.issued || ordinal >= self.descriptor.lease_slots() {
            return Err(Refusal::Range);
        }
        if ordinal != 0 && !self.slots[ordinal - 1].published {
            return Err(Refusal::Unpublished);
        }
        let nonce = self.issuer.mint()?;
        if ordinal == 0 {
            self.allocate_metadata();
        }
        let offset = ordinal
            .checked_mul(self.descriptor.width())
            .ok_or(Refusal::Range)?;
        let end = offset
            .checked_add(self.descriptor.width())
            .ok_or(Refusal::Range)?;
        if end > self.descriptor.payload_bytes as usize {
            return Err(Refusal::Range);
        }
        if self.backing.is_none() {
            self.backing = Some(std::mem::ManuallyDrop::new(
                self.executor
                    .allocate(self.descriptor.payload_bytes as usize),
            ));
        }
        self.slots[ordinal] = Slot {
            nonce,
            generation: 1,
            live: true,
            published: false,
            borrows: 0,
        };
        self.issued += 1;
        self.live += 1;
        Ok(Lease {
            lifetime: std::marker::PhantomData,
            issuer: self.issuer_key(),
            domain: self.domain,
            module: self.module_key(),
            slot: ordinal,
            nonce,
            generation: 1,
            offset,
            len: self.descriptor.width(),
        })
    }
    fn allocate_metadata(&mut self) {
        let count = self.descriptor.lease_slots();
        self.slots
            .try_reserve_exact(count)
            .expect("fusion metadata allocation invariant failure");
        self.views
            .try_reserve_exact(MAX_VIEWS)
            .expect("fusion view metadata allocation invariant failure");
        assert_eq!(
            self.slots.capacity(),
            count,
            "fusion slot capacity invariant"
        );
        assert_eq!(
            self.views.capacity(),
            MAX_VIEWS,
            "fusion view capacity invariant"
        );
        self.slots.resize(
            count,
            Slot {
                nonce: 0,
                generation: 0,
                live: false,
                published: false,
                borrows: 0,
            },
        );
        self.views.resize(
            MAX_VIEWS,
            View {
                nonce: 0,
                slot: 0,
                generation: 0,
                active: false,
            },
        );
    }
    fn validate(&self, lease: &Lease<'_>) -> Result<&Slot, Refusal> {
        if lease.issuer != self.issuer_key()
            || lease.domain != self.domain
            || lease.module != self.module_key()
        {
            return Err(Refusal::Identity);
        }
        let slot = self.slots.get(lease.slot).ok_or(Refusal::Identity)?;
        if !slot.live || slot.nonce != lease.nonce || slot.generation != lease.generation {
            return Err(Refusal::Identity);
        }
        if lease.offset
            != lease
                .slot
                .checked_mul(self.descriptor.width())
                .ok_or(Refusal::Range)?
            || lease.len != self.descriptor.width()
        {
            return Err(Refusal::Range);
        }
        Ok(slot)
    }
    /// Exact whole-row push/commit, not a borrow or permission for another copy.
    pub(crate) fn publish(&mut self, lease: &Lease<'_>) -> Result<(), Refusal> {
        if self.phase != Phase::Building {
            return Err(Refusal::Phase);
        }
        self.validate(lease)?;
        if self.slots[lease.slot].published {
            return Err(Refusal::Unpublished);
        }
        self.slots[lease.slot].published = true;
        Ok(())
    }
    pub(crate) fn complete(&mut self) -> Result<(), Refusal> {
        if self.phase != Phase::Building
            || self.issued != self.descriptor.lease_slots()
            || self.slots.len() != self.descriptor.lease_slots()
            || self.slots.iter().any(|slot| !slot.published)
        {
            return Err(Refusal::Unpublished);
        }
        self.phase = Phase::Complete;
        self.construction_pin = false;
        self.release_if_last();
        Ok(())
    }
    pub(crate) fn abort(&mut self) -> Result<(), Refusal> {
        if !matches!(self.phase, Phase::Unstarted | Phase::Building) {
            return Err(Refusal::Phase);
        }
        self.phase = Phase::Aborted;
        Ok(())
    }
    /// An executor calls this after canonical partial-row/Vec/String cleanup;
    /// the reference additionally requires every issued byte child be settled.
    pub(crate) fn settle_abort(&mut self) -> Result<(), Refusal> {
        if self.phase != Phase::Aborted
            || self.live != 0
            || self.views.iter().any(|view| view.active)
        {
            return Err(Refusal::Phase);
        }
        self.construction_pin = false;
        self.release_if_last();
        self.phase = Phase::Settled;
        Ok(())
    }
    pub(crate) fn borrow(&mut self, lease: &Lease<'_>) -> Result<Borrow<'a>, Refusal> {
        if self.phase != Phase::Complete {
            return Err(Refusal::Phase);
        }
        self.validate(lease)?;
        let view = self
            .views
            .iter()
            .position(|view| !view.active)
            .ok_or(Refusal::Metadata)?;
        let count = self.slots[lease.slot]
            .borrows
            .checked_add(1)
            .ok_or(Refusal::Exhausted)?;
        let nonce = self.issuer.mint()?;
        self.views[view] = View {
            nonce,
            slot: lease.slot,
            generation: lease.generation,
            active: true,
        };
        self.slots[lease.slot].borrows = count;
        Ok(Borrow {
            lifetime: std::marker::PhantomData,
            issuer: self.issuer_key(),
            domain: self.domain,
            module: self.module_key(),
            view,
            nonce,
            slot: lease.slot,
            generation: lease.generation,
            offset: lease.offset,
            len: lease.len,
        })
    }
    /// Resolve a current borrowed range before any executor payload access.
    pub(crate) fn read_range(
        &self,
        view: &Borrow<'_>,
        relative: usize,
        length: usize,
    ) -> Result<(usize, usize), Refusal> {
        if self.phase != Phase::Complete {
            return Err(Refusal::Phase);
        }
        if view.issuer != self.issuer_key()
            || view.domain != self.domain
            || view.module != self.module_key()
        {
            return Err(Refusal::Identity);
        }
        let stored = self.views.get(view.view).ok_or(Refusal::Identity)?;
        let slot = self.slots.get(view.slot).ok_or(Refusal::Identity)?;
        if !stored.active
            || stored.nonce != view.nonce
            || stored.slot != view.slot
            || stored.generation != view.generation
            || !slot.live
            || slot.generation != view.generation
            || view.offset
                != view
                    .slot
                    .checked_mul(self.descriptor.width())
                    .ok_or(Refusal::Range)?
            || view.len != self.descriptor.width()
        {
            return Err(Refusal::Identity);
        }
        if relative
            .checked_add(length)
            .is_none_or(|end| end > view.len)
        {
            return Err(Refusal::Range);
        }
        Ok((
            view.offset.checked_add(relative).ok_or(Refusal::Range)?,
            length,
        ))
    }
    /// A borrowed callback cannot return a borrow of the Owner: its argument
    /// lifetime is local to this call. No raw backing owner is exported.
    pub(crate) fn with_borrowed_range<T>(
        &self,
        view: &Borrow<'_>,
        apply: impl FnOnce(&B::Owner, std::ops::Range<usize>) -> T,
    ) -> Result<T, Refusal> {
        let (offset, length) = self.read_range(view, 0, view.len)?;
        let owner = self.backing.as_ref().ok_or(Refusal::Identity)?;
        Ok(apply(owner, offset..offset + length))
    }
    pub(crate) fn end_borrow(&mut self, view: &mut Borrow<'_>) -> Result<(), Refusal> {
        if view.issuer != self.issuer_key()
            || view.domain != self.domain
            || view.module != self.module_key()
        {
            return Err(Refusal::Identity);
        }
        let stored = self.views.get(view.view).ok_or(Refusal::Identity)?;
        if !stored.active
            || stored.nonce != view.nonce
            || stored.slot != view.slot
            || stored.generation != view.generation
        {
            return Err(Refusal::Identity);
        }
        let slot = self.slots.get_mut(view.slot).ok_or(Refusal::Identity)?;
        if !slot.live
            || slot.generation != view.generation
            || slot.borrows == 0
            || view.offset != view.slot * self.descriptor.width()
            || view.len != self.descriptor.width()
        {
            return Err(Refusal::Identity);
        }
        slot.borrows -= 1;
        self.views[view.view].active = false;
        view.nonce = 0;
        Ok(())
    }
    pub(crate) fn move_owner(&mut self, lease: &mut Lease<'_>) -> Result<(), Refusal> {
        if self.phase != Phase::Complete {
            return Err(Refusal::Phase);
        }
        if self.validate(lease)?.borrows != 0 {
            return Err(Refusal::Borrowed);
        }
        let generation = lease.generation.checked_add(1).ok_or(Refusal::Exhausted)?;
        let nonce = self.issuer.mint()?;
        self.slots[lease.slot].generation = generation;
        self.slots[lease.slot].nonce = nonce;
        lease.generation = generation;
        lease.nonce = nonce;
        Ok(())
    }
    pub(crate) fn drop_owner(&mut self, lease: &mut Lease<'_>) -> Result<(), Refusal> {
        if !matches!(self.phase, Phase::Complete | Phase::Aborted) {
            return Err(Refusal::Phase);
        }
        if self.validate(lease)?.borrows != 0 {
            return Err(Refusal::Borrowed);
        }
        self.slots[lease.slot].live = false;
        self.live -= 1;
        lease.nonce = 0;
        self.release_if_last();
        Ok(())
    }
    fn release_if_last(&mut self) {
        if self.live == 0 && !self.construction_pin {
            if let Some(owner) = self.backing.take() {
                self.executor
                    .release(std::mem::ManuallyDrop::into_inner(owner));
            }
            self.phase = Phase::Settled;
        }
    }
}
#[cfg(test)]
mod tests;

/// Shared private range lifetime core; no source or backend selection.
pub(crate) mod owner_core;

impl<B: Backing> Drop for Domain<'_, B> {
    fn drop(&mut self) {
        if let Some(owner) = self.backing.take() {
            self.executor.release(std::mem::ManuallyDrop::into_inner(owner));
        }
    }
}
