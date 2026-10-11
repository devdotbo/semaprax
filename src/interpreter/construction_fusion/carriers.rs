//! Byte carriers preserve ordinary backing and keep private leases opaque.
use super::super::{Arc, BorrowedRoot, Flow, Utf8Backing, ValueId};
use super::{Lease, Registry, View};
#[derive(Debug, PartialEq)]
pub(in crate::interpreter) enum Backing {
    Ordinary(Arc<[u8]>),
    Leased(Lease),
    // Affine token retained by its exact invocation registry; never cloned.
    Partition(super::super::byte_partition::Owner),
}
impl From<Arc<[u8]>> for Backing {
    fn from(bytes: Arc<[u8]>) -> Self {
        Self::Ordinary(bytes)
    }
}
impl Backing {
    pub(in crate::interpreter) fn borrow_alias(&self) -> Result<Self, Flow> {
        match self {
            Self::Ordinary(bytes) => Ok(Self::Ordinary(Arc::clone(bytes))),
            Self::Leased(lease) => Ok(Self::Leased(lease.clone())),
            Self::Partition(_) => Err(Flow::Guard(
                "affine partition owner requires an authenticated loan descriptor",
            )),
        }
    }
    pub(in crate::interpreter) fn ordinary(&self) -> &Arc<[u8]> {
        match self {
            Self::Ordinary(bytes) => bytes,
            Self::Leased(_) | Self::Partition(_) => {
                panic!("private leased backing reached an ordinary adapter")
            }
        }
    }
    pub(in crate::interpreter) fn ordinary_mut(&mut self) -> &mut Arc<[u8]> {
        match self {
            Self::Ordinary(bytes) => bytes,
            Self::Leased(_) | Self::Partition(_) => {
                panic!("private leased backing reached an ordinary store")
            }
        }
    }
}
// These legacy-only adapters never recover a lease as a raw byte pointer.
impl AsRef<[u8]> for Backing {
    fn as_ref(&self) -> &[u8] {
        self.ordinary().as_ref()
    }
}
impl std::ops::Deref for Backing {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.ordinary().as_ref()
    }
}
#[derive(Debug, PartialEq)]
pub(in crate::interpreter) struct OwnedBytesValue {
    pub(in crate::interpreter) allocation: u32,
    pub(in crate::interpreter) bytes: Backing,
}
#[derive(Clone, Debug, PartialEq)]
pub(in crate::interpreter) struct BorrowedSliceValue {
    pub(in crate::interpreter) invocation_root: BorrowedRoot,
    pub(in crate::interpreter) backing: Option<Utf8Backing>,
    pub(in crate::interpreter) start: usize,
    pub(in crate::interpreter) end: usize,
    pub(in crate::interpreter) lease: Option<View>,
    pub(in crate::interpreter) partition: Option<super::super::byte_partition::loans::ViewId>,
}
impl BorrowedSliceValue {
    pub(in crate::interpreter) fn whole(invocation_root: ValueId, backing: Arc<[u8]>) -> Self {
        Self::from_view(invocation_root.into(), backing.into())
    }
    pub(in crate::interpreter) fn from_view(
        invocation_root: BorrowedRoot,
        backing: Utf8Backing,
    ) -> Self {
        let end = backing.len();
        Self {
            invocation_root,
            backing: Some(backing),
            start: 0,
            end,
            lease: None,
            partition: None,
        }
    }
    pub(in crate::interpreter) fn bytes(&self) -> &[u8] {
        assert!(
            self.lease.is_none() && self.partition.is_none(),
            "private view reached an ordinary adapter"
        );
        &self.backing.as_ref().expect("ordinary slice backing")[self.start..self.end]
    }
    pub(in crate::interpreter) fn range(&self, start: usize, end: usize) -> Self {
        Self {
            invocation_root: self.invocation_root.clone(),
            backing: self.backing.clone(),
            start: self.start + start,
            end: self.start + end,
            lease: self.lease,
            partition: self.partition,
        }
    }
    pub(in crate::interpreter) fn read<T>(
        &self,
        registry: &mut Registry<'_>,
        consumer: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, Flow> {
        if self.partition.is_some() {
            return Err(Flow::Guard(
                "partition view requires its ordinary receiving registry",
            ));
        }
        match self.lease {
            Some(view) => registry.read(view, self.start, self.end, consumer),
            None => Ok(consumer(self.bytes())),
        }
    }
}
impl OwnedBytesValue {
    pub(in crate::interpreter) fn borrow_alias(&self) -> Result<Self, Flow> {
        Ok(Self {
            allocation: self.allocation,
            bytes: self.bytes.borrow_alias()?,
        })
    }
    pub(in crate::interpreter) fn read<T>(
        &self,
        registry: &mut Registry<'_>,
        consumer: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, Flow> {
        match &self.bytes {
            Backing::Ordinary(bytes) => Ok(consumer(bytes)),
            Backing::Leased(lease) => registry.read_owner(lease, consumer),
            Backing::Partition(_) => Err(Flow::Guard(
                "partition owner requires its ordinary receiving registry",
            )),
        }
    }
}

impl OwnedBytesValue {
    pub(in crate::interpreter) fn partition(
        allocation: u32,
        owner: super::super::byte_partition::Owner,
    ) -> Self {
        Self {
            allocation,
            bytes: Backing::Partition(owner),
        }
    }
    pub(in crate::interpreter) fn borrow_partition_slice<
        R: super::super::byte_partition::InvocationRuntime,
    >(
        &self,
        receiver: &mut super::super::byte_partition::loans::Loans<'_, R>,
        loan: crate::loan_plan::LoanId,
        root: ValueId,
    ) -> Result<BorrowedSliceValue, Flow> {
        let Backing::Partition(owner) = &self.bytes else {
            return Err(Flow::Guard(
                "partition slice producer requires an affine partition owner",
            ));
        };
        let end = receiver.extent(owner).map_err(super::guard)?;
        let view = receiver.borrow(owner, loan).map_err(super::guard)?;
        Ok(BorrowedSliceValue {
            invocation_root: root.into(),
            backing: None,
            start: 0,
            end,
            lease: None,
            partition: Some(view),
        })
    }
    pub(in crate::interpreter) fn move_partition<
        R: super::super::byte_partition::InvocationRuntime,
    >(
        &mut self,
        receiver: &mut super::super::byte_partition::loans::Loans<'_, R>,
    ) -> Result<(), Flow> {
        let Backing::Partition(owner) = &mut self.bytes else {
            return Err(Flow::Guard(
                "partition move requires an affine partition owner",
            ));
        };
        receiver.moved(owner).map_err(super::guard)
    }
    pub(in crate::interpreter) fn store_partition<
        R: super::super::byte_partition::InvocationRuntime,
    >(
        &mut self,
        receiver: &mut super::super::byte_partition::loans::Loans<'_, R>,
        index: usize,
        bytes: &[u8],
    ) -> Result<(), Flow> {
        let Backing::Partition(owner) = &mut self.bytes else {
            return Err(Flow::Guard(
                "partition store requires an affine partition owner",
            ));
        };
        receiver.store(owner, index, bytes).map_err(super::guard)
    }
    pub(in crate::interpreter) fn settle_partition<
        R: super::super::byte_partition::InvocationRuntime,
    >(
        &mut self,
        receiver: &mut super::super::byte_partition::loans::Loans<'_, R>,
    ) -> Result<(), Flow> {
        let Backing::Partition(owner) = &mut self.bytes else {
            return Err(Flow::Guard(
                "partition finalizer requires an affine partition owner",
            ));
        };
        receiver.settle(owner).map_err(super::guard)
    }
}
impl BorrowedSliceValue {
    pub(in crate::interpreter) fn read_partition<
        T,
        R: super::super::byte_partition::InvocationRuntime,
    >(
        &self,
        receiver: &mut super::super::byte_partition::loans::Loans<'_, R>,
        f: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, Flow> {
        let view = self
            .partition
            .ok_or(Flow::Guard("partition view descriptor is unavailable"))?;
        receiver
            .read_range(view, self.start, self.end, f)
            .map_err(super::guard)
    }
}
