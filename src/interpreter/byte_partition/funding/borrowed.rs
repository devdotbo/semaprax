//! Lifetime-safe lending of the one stack-held invocation account.
//! This trait is crate-private and its sole implementation is Account. It
//! erases no lifetime: every handle still borrows the actual account.
use super::Account;
use crate::construction_fusion::domain::{Refusal, owner_core::Runtime};
use std::alloc::Layout;

pub(crate) trait ByteAccount {
    fn identity(&self) -> u64;
    fn counters(&self) -> (u32, u64);
    fn logical_refused(&self) -> bool;
    fn poison(&self);
    fn active(&self) -> bool;
    fn reserve(&self, layout: Layout) -> Result<(), Refusal>;
    fn charge(&self, extent: usize) -> Result<u32, AllocationError>;
}
#[derive(Debug, PartialEq)]
pub(crate) enum AllocationError {
    Logical,
    Authority(Refusal),
}
#[derive(Clone, Copy)]
pub(crate) struct BorrowedAccount<'a>(&'a dyn ByteAccount);
impl Account<'_> {
    pub(crate) fn borrowed(&self) -> BorrowedAccount<'_> {
        BorrowedAccount(self)
    }
}
impl ByteAccount for Account<'_> {
    fn poison(&self) {
        self.failed.set(true);
    }
    fn active(&self) -> bool {
        !self.failed.get()
    }
    fn identity(&self) -> u64 {
        self.identity
    }
    fn counters(&self) -> (u32, u64) {
        let logical = self.logical.borrow();
        (*logical.events, *logical.payload)
    }
    fn logical_refused(&self) -> bool {
        self.logical_refused.get()
    }
    fn reserve(&self, layout: Layout) -> Result<(), Refusal> {
        Account::reserve(self, layout)
    }
    fn charge(&self, extent: usize) -> Result<u32, AllocationError> {
        if self.failed.get() {
            return Err(AllocationError::Authority(Refusal::Metadata));
        }
        let mut logical = self.logical.borrow_mut();
        let next = (*logical.payload).checked_add(extent as u64);
        if extent > crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES as usize
            || *logical.events >= crate::byte_data_capacity::MAX_BYTES_COPY_SITES
            || next.is_none_or(|v| v > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES)
        {
            self.logical_refused.set(true);
            self.failed.set(true);
            return Err(AllocationError::Logical);
        }
        self.reserve(
            Layout::array::<u8>(extent)
                .map_err(|_| AllocationError::Authority(Refusal::Metadata))?,
        )
        .map_err(AllocationError::Authority)?;
        *logical.events += 1;
        *logical.payload = next.unwrap();
        Ok(*logical.events)
    }
}
impl BorrowedAccount<'_> {
    pub(crate) fn poison(self) {
        self.0.poison();
    }
    pub(crate) fn active(self) -> bool {
        self.0.active()
    }
    pub(crate) fn identity(self) -> u64 {
        self.0.identity()
    }
    pub(crate) fn logical_refused(self) -> bool {
        self.0.logical_refused()
    }
    pub(crate) fn counters(self) -> (u32, u64) {
        self.0.counters()
    }
    pub(crate) fn allocate_zeroed(
        self,
        extent: usize,
    ) -> Result<(u32, Box<[u8]>), AllocationError> {
        let allocation = self.0.charge(extent)?;
        let mut handle = self;
        Ok((allocation, handle.allocate(extent)))
    }
    pub(crate) fn copy(self, bytes: &[u8]) -> Result<(u32, Box<[u8]>), AllocationError> {
        let (allocation, mut owner) = self.allocate_zeroed(bytes.len())?;
        owner.copy_from_slice(bytes);
        Ok((allocation, owner))
    }
}
impl super::super::InvocationRuntime for BorrowedAccount<'_> {
    fn held_account(&self) -> Self {
        *self
    }
}
impl Runtime for BorrowedAccount<'_> {
    type Owner = Box<[u8]>;
    fn reserve_metadata(&mut self, layout: Layout) -> Result<(), Refusal> {
        self.0.reserve(layout)
    }
    fn charge_clone(&mut self, extent: usize) -> Result<(), Refusal> {
        self.0.charge(extent).map(|_| ()).map_err(|e| match e {
            AllocationError::Logical => Refusal::Metadata,
            AllocationError::Authority(e) => e,
        })
    }
    fn allocate(&mut self, extent: usize) -> Self::Owner {
        // Identical exact boxed allocation policy to the typed borrowed handle.
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(extent)
            .expect("ordinary partition backing allocation invariant failure");
        assert_eq!(bytes.capacity(), extent);
        bytes.resize(extent, 0);
        bytes.into_boxed_slice()
    }
    fn bytes(owner: &Self::Owner) -> &[u8] {
        owner
    }
    fn bytes_mut(owner: &mut Self::Owner) -> &mut [u8] {
        owner
    }
    fn release(&mut self, owner: Self::Owner) {
        drop(owner);
    }
}
