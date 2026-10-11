//! Executor-neutral held backing/range core. No descriptor or source authority.
//! The executor debits its REAL invocation account before metadata/clone allocation.
use super::{Issuer, ModuleIdentity, Refusal, MAX_VIEWS};
use std::{alloc::Layout, marker::PhantomData, ops::Range};

/// Implementations own unique backing, expose its actual extent, and fail-stop
/// on physical allocation failure. Reservations are monotonic, never refunded.
/// `charge_clone` uses ordinary backing-event/payload counters (32 / 2 MiB),
/// not a second range-domain allowance.
pub(crate) trait Runtime {
    type Owner;
    fn reserve_metadata(&mut self, layout: Layout) -> Result<(), Refusal>;
    fn charge_clone(&mut self, extent: usize) -> Result<(), Refusal>;
    fn allocate(&mut self, extent: usize) -> Self::Owner;
    fn bytes(owner: &Self::Owner) -> &[u8];
    fn bytes_mut(owner: &mut Self::Owner) -> &mut [u8];
    fn release(&mut self, owner: Self::Owner);
}
#[derive(Clone, Copy)]
struct Slot {
    offset: usize,
    len: usize,
    nonce: u64,
    generation: u64,
    live: bool,
    borrows: usize,
}
#[derive(Clone, Copy)]
struct View {
    slot: usize,
    offset: usize,
    len: usize,
    nonce: u64,
    generation: u64,
    live: bool,
}
/// Affine private token, not an ABI carrier. All fields are authenticated before
/// obtaining a backing reference. Child authority never becomes whole authority.
#[must_use]
pub(crate) struct Child<'a> {
    held: PhantomData<&'a Issuer>,
    issuer: usize,
    module: usize,
    domain: u64,
    slot: usize,
    nonce: u64,
    generation: u64,
}
#[must_use]
pub(crate) struct Loan<'a> {
    held: PhantomData<&'a Issuer>,
    issuer: usize,
    module: usize,
    domain: u64,
    view: usize,
    nonce: u64,
}
/// Must outlive every carrier/container using its tokens. Container cleanup
/// settles its children in canonical order; a popped child stays in this held
/// context. Abandoning the context releases backing, never a language result.
pub(crate) struct Core<'a, R: Runtime> {
    issuer: &'a mut Issuer,
    module: &'a ModuleIdentity,
    domain: u64,
    slots: Box<[Slot]>,
    views: Box<[View]>,
    backing: Option<R::Owner>,
    runtime: R,
    live: usize,
    width: usize,
    extent: usize,
    failure: Option<Refusal>,
}
impl<'a, R: Runtime> Core<'a, R> {
    /// All allocations made here: held context, exact slot/view backing, and
    /// the returned complete carrier inventory. No per-child allocation.
    pub(crate) fn metadata_layouts(count: usize) -> Result<[Layout; 7], Refusal> {
        Ok([
            Layout::new::<Self>(),
            Layout::array::<Slot>(count).map_err(|_| Refusal::Metadata)?,
            Layout::array::<View>(MAX_VIEWS).map_err(|_| Refusal::Metadata)?,
            Layout::array::<Child<'a>>(count).map_err(|_| Refusal::Metadata)?,
            Layout::new::<Vec<Child<'a>>>(),
            // Temporary table headers remain live while the context is formed.
            Layout::new::<Vec<Slot>>(),
            Layout::new::<Vec<View>>(),
        ])
    }
    /// The caller supplies a separately authenticated ordinary WHOLE owner.
    /// This core accepts no Child overload. On EVERY precommit refusal that
    /// exact original owner is returned; no partial child inventory is exposed.
    pub(crate) fn partition(
        owner: R::Owner,
        count: usize,
        width: usize,
        issuer: &'a mut Issuer,
        module: &'a ModuleIdentity,
        mut runtime: R,
    ) -> Result<(Self, Vec<Child<'a>>), (Refusal, R::Owner)> {
        let preparation = (|| {
            if width == 0 {
                return Err(Refusal::Range);
            }
            let extent = count.checked_mul(width).ok_or(Refusal::Range)?;
            if extent != R::bytes(&owner).len()
                || extent > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES as usize
            {
                return Err(Refusal::Range);
            }
            if count > crate::vec_ops::MAX_CAPACITY as usize {
                return Err(Refusal::Metadata);
            }
            for layout in Self::metadata_layouts(count)? {
                runtime.reserve_metadata(layout)?;
            }
            // Reserve all nonreusable identities transactionally BEFORE tables.
            let end = issuer
                .next
                .checked_add(count as u64 + 1)
                .ok_or(Refusal::Exhausted)?;
            let domain = issuer.next;
            issuer.next = end;
            Ok(domain)
        })();
        let domain = match preparation {
            Ok(d) => d,
            Err(e) => return Err((e, owner)),
        };
        let mut slots = exact_vec(count);
        let mut children = exact_vec(count);
        let mut views = exact_vec(MAX_VIEWS);
        let issuer_key = std::ptr::from_ref(&*issuer) as usize;
        let module_key = std::ptr::from_ref(module) as usize;
        for i in 0..count {
            let nonce = domain + 1 + i as u64;
            slots.push(Slot {
                offset: i * width,
                len: width,
                nonce,
                generation: 1,
                live: true,
                borrows: 0,
            });
            children.push(Child {
                held: PhantomData,
                issuer: issuer_key,
                module: module_key,
                domain,
                slot: i,
                nonce,
                generation: 1,
            });
        }
        views.resize(
            MAX_VIEWS,
            View {
                slot: 0,
                offset: 0,
                len: 0,
                nonce: 0,
                generation: 0,
                live: false,
            },
        );
        let mut core = Self {
            issuer,
            module,
            domain,
            slots: slots.into_boxed_slice(),
            views: views.into_boxed_slice(),
            backing: Some(owner),
            runtime,
            live: count,
            width,
            extent: count * width,
            failure: None,
        };
        core.release_last();
        Ok((core, children))
    }
    fn identity(&self, issuer: usize, module: usize, domain: u64) -> Result<(), Refusal> {
        if issuer != std::ptr::from_ref(&*self.issuer) as usize
            || module != std::ptr::from_ref(self.module) as usize
            || domain != self.domain
        {
            return Err(Refusal::Identity);
        }
        Ok(())
    }
    fn slot(&self, child: &Child<'_>) -> Result<&Slot, Refusal> {
        self.identity(child.issuer, child.module, child.domain)?;
        let s = self.checked_slot(child.slot)?;
        if !s.live || s.nonce != child.nonce || s.generation != child.generation {
            return Err(Refusal::Identity);
        }
        Ok(s)
    }
    fn checked_slot(&self, index: usize) -> Result<&Slot, Refusal> {
        let s = self.slots.get(index).ok_or(Refusal::Identity)?;
        let offset = index.checked_mul(self.width).ok_or(Refusal::Range)?;
        if s.offset != offset
            || s.len != self.width
            || offset
                .checked_add(s.len)
                .is_none_or(|end| end > self.extent)
        {
            return Err(Refusal::Range);
        }
        Ok(s)
    }
    fn active(&self) -> Result<(), Refusal> {
        self.failure.map_or(Ok(()), Err)
    }
    fn select<T>(&mut self, result: Result<T, Refusal>) -> Result<T, Refusal> {
        if let Err(e) = result {
            if self.failure.is_none() {
                self.failure = Some(e);
            }
        }
        result
    }
    pub(crate) fn failure(&self) -> Option<Refusal> {
        self.failure
    }
    pub(crate) fn borrow(
        &mut self,
        child: &Child<'_>,
        range: Range<usize>,
    ) -> Result<Loan<'a>, Refusal> {
        let result = (|| {
            self.active()?;
            let s = *self.slot(child)?;
            if range.start > range.end || range.end > s.len {
                return Err(Refusal::Range);
            }
            let index = self
                .views
                .iter()
                .position(|v| !v.live)
                .ok_or(Refusal::Metadata)?;
            let borrows = s.borrows.checked_add(1).ok_or(Refusal::Exhausted)?;
            let nonce = self.issuer.mint()?;
            self.views[index] = View {
                slot: child.slot,
                offset: s.offset + range.start,
                len: range.end - range.start,
                nonce,
                generation: s.generation,
                live: true,
            };
            self.slots[child.slot].borrows = borrows;
            Ok(Loan {
                held: PhantomData,
                issuer: child.issuer,
                module: child.module,
                domain: child.domain,
                view: index,
                nonce,
            })
        })();
        self.select(result)
    }
    fn view(&self, loan: &Loan<'_>) -> Result<&View, Refusal> {
        self.identity(loan.issuer, loan.module, loan.domain)?;
        let v = self.views.get(loan.view).ok_or(Refusal::Identity)?;
        let s = self.checked_slot(v.slot)?;
        if !v.live
            || v.nonce != loan.nonce
            || !s.live
            || s.borrows == 0
            || s.generation != v.generation
            || v.offset < s.offset
            || v.offset
                .checked_add(v.len)
                .is_none_or(|e| e > s.offset + s.len)
        {
            return Err(Refusal::Identity);
        }
        Ok(v)
    }
    pub(crate) fn read<T>(
        &mut self,
        loan: &Loan<'_>,
        apply: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, Refusal> {
        let result = (|| {
            self.active()?;
            let v = self.view(loan)?;
            let bytes = R::bytes(self.backing.as_ref().ok_or(Refusal::Identity)?);
            Ok(apply(&bytes[v.offset..v.offset + v.len]))
        })();
        self.select(result)
    }
    /// Cleanup remains possible after sticky failure; it cannot replace it.
    pub(crate) fn end_borrow(&mut self, loan: &mut Loan<'_>) -> Result<(), Refusal> {
        let result = (|| {
            let v = *self.view(loan)?;
            self.slots[v.slot].borrows -= 1;
            self.views[loan.view].live = false;
            loan.nonce = 0;
            Ok(())
        })();
        self.select(result)
    }
    pub(crate) fn move_owner(&mut self, child: &mut Child<'_>) -> Result<(), Refusal> {
        let result = (|| {
            self.active()?;
            let s = *self.slot(child)?;
            if s.borrows != 0 {
                return Err(Refusal::Borrowed);
            }
            let generation = s.generation.checked_add(1).ok_or(Refusal::Exhausted)?;
            let nonce = self.issuer.mint()?;
            self.slots[child.slot].generation = generation;
            self.slots[child.slot].nonce = nonce;
            child.generation = generation;
            child.nonce = nonce;
            Ok(())
        })();
        self.select(result)
    }
    /// Store renews precisely this child's generation, preserving disjointness.
    pub(crate) fn store(
        &mut self,
        child: &mut Child<'_>,
        index: usize,
        byte: u8,
    ) -> Result<(), Refusal> {
        let result = (|| {
            self.active()?;
            let s = *self.slot(child)?;
            if s.borrows != 0 {
                return Err(Refusal::Borrowed);
            }
            if index >= s.len {
                return Err(Refusal::Range);
            }
            // Exhaustion refusal occurs before the physical write.
            self.move_owner(child)?;
            R::bytes_mut(self.backing.as_mut().ok_or(Refusal::Identity)?)[s.offset + index] = byte;
            Ok(())
        })();
        self.select(result)
    }
    /// Fresh ordinary backing, with the REAL ordinary quota debit before allocate.
    /// Result ownership belongs to ordinary canonical cleanup, not this context.
    pub(crate) fn deep_clone(&mut self, child: &Child<'_>) -> Result<R::Owner, Refusal> {
        let result = (|| {
            self.active()?;
            let s = *self.slot(child)?;
            self.runtime.charge_clone(s.len)?;
            let mut output = self.runtime.allocate(s.len);
            assert_eq!(
                R::bytes(&output).len(),
                s.len,
                "clone allocation extent invariant"
            );
            let input = R::bytes(self.backing.as_ref().ok_or(Refusal::Identity)?);
            R::bytes_mut(&mut output).copy_from_slice(&input[s.offset..s.offset + s.len]);
            Ok(output)
        })();
        self.select(result)
    }
    pub(crate) fn drop_owner(&mut self, child: &mut Child<'_>) -> Result<(), Refusal> {
        let result = (|| {
            if self.slot(child)?.borrows != 0 {
                return Err(Refusal::Borrowed);
            }
            self.slots[child.slot].live = false;
            self.live -= 1;
            child.nonce = 0;
            self.release_last();
            Ok(())
        })();
        self.select(result)
    }
    fn release_last(&mut self) {
        if self.live == 0 {
            if let Some(owner) = self.backing.take() {
                self.runtime.release(owner);
            }
        }
    }
}
impl<R: Runtime> Drop for Core<'_, R> {
    fn drop(&mut self) {
        // Administrative abandonment of this held context; no result publication,
        // semantic status selection or synthesized language finalizer order.
        if let Some(owner) = self.backing.take() {
            self.runtime.release(owner);
        }
    }
}
fn exact_vec<T>(count: usize) -> Vec<T> {
    let mut out = Vec::new();
    out.try_reserve_exact(count)
        .expect("range metadata allocation invariant failure");
    assert_eq!(
        out.capacity(),
        count,
        "range metadata spare capacity invariant"
    );
    out
}
#[cfg(test)]
mod tests;
