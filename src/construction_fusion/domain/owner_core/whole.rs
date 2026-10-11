//! Closed whole-owner holding for ordinary interpreter views. This is not a
//! partition operation: an empty whole is valid and still has one affine owner.
use super::{Child, Core, IssuerHolder, ModuleHolder, Runtime, Slot, View, exact_vec};
use crate::construction_fusion::domain::{Issuer, ModuleIdentity, Refusal};
use std::{alloc::Layout, marker::PhantomData};
impl<R: Runtime> Core<'static, R> {
    #[cfg(test)]
    pub(crate) fn whole_test_table_units() -> [Layout; 4] {
        // Unit layouts only; the owning registry test independently supplies
        // whole-slot/view counts and the full overlapping request formula.
        [
            Layout::new::<Slot>(),
            Layout::new::<View>(),
            Layout::new::<Vec<Slot>>(),
            Layout::new::<Vec<View>>(),
        ]
    }

    pub(crate) fn hold_whole_ordinary(
        owner: R::Owner,
        mut issuer: Issuer,
        module: ModuleIdentity,
        mut runtime: R,
        capacity: &crate::interpreter::byte_partition::authority::LoanCapacity<'_>,
    ) -> Result<(Self, Vec<Child<'static>>), (Refusal, R::Owner)> {
        let extent = R::bytes(&owner).len();
        let preparation = (|| {
            if extent > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES as usize {
                return Err(Refusal::Range);
            }
            let next = issuer.next.checked_add(2).ok_or(Refusal::Exhausted)?;
            for layout in Self::ordinary_metadata_layouts(1, capacity)? {
                runtime.reserve_metadata(layout)?;
            }
            runtime.reserve_metadata(Layout::new::<Issuer>())?;
            runtime.reserve_metadata(Layout::new::<ModuleIdentity>())?;
            Ok(next)
        })();
        let next = match preparation {
            Ok(next) => next,
            Err(error) => return Err((error, owner)),
        };
        let domain = issuer.next;
        issuer.next = next;
        let issuer = IssuerHolder::Owned(Box::new(issuer));
        let module = ModuleHolder::Owned(Box::new(module));
        let issuer_key = std::ptr::from_ref(issuer.get()) as usize;
        let module_key = std::ptr::from_ref(module.get()) as usize;
        let nonce = domain + 1;
        let mut slots = exact_vec(1);
        slots.push(Slot {
            offset: 0,
            len: extent,
            nonce,
            generation: 1,
            live: true,
            borrows: 0,
        });
        let mut children = exact_vec(1);
        children.push(Child {
            held: PhantomData,
            issuer: issuer_key,
            module: module_key,
            domain,
            slot: 0,
            nonce,
            generation: 1,
        });
        let mut views = exact_vec(capacity.slots());
        views.resize(
            capacity.slots(),
            View {
                slot: 0,
                offset: 0,
                len: 0,
                nonce: 0,
                generation: 0,
                live: false,
            },
        );
        Ok((
            Self {
                issuer,
                module,
                domain,
                slots: slots.into_boxed_slice(),
                views: views.into_boxed_slice(),
                backing: Some(owner),
                runtime,
                live: 1,
                width: extent,
                extent,
                failure: None,
                whole: true,
            },
            children,
        ))
    }
    /// A partition child can never become a whole owner. Move-out authenticates
    /// this exact one-owner context and requires every physical view settled.
    pub(crate) fn take_whole(&mut self, child: &mut Child<'_>) -> Result<R::Owner, Refusal> {
        let result = (|| {
            self.active()?;
            let slot = self.slot(child)?;
            if !self.whole
                || self.live != 1
                || child.slot != 0
                || self.slots.len() != 1
                || slot.offset != 0
                || slot.len != self.extent
            {
                return Err(Refusal::Identity);
            }
            if slot.borrows != 0 {
                return Err(Refusal::Borrowed);
            }
            let owner = self.backing.take().ok_or(Refusal::Identity)?;
            self.slots[0].live = false;
            self.live = 0;
            child.nonce = 0;
            Ok(owner)
        })();
        self.select(result)
    }
}
