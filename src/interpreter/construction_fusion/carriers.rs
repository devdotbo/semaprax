//! Byte carriers preserve ordinary backing and keep private leases opaque.
use super::super::{Arc, BorrowedRoot, Flow, Utf8Backing, ValueId};
use super::{Lease, Registry, View};
#[derive(Clone, Debug, PartialEq)]
pub(in crate::interpreter) enum Backing {
    Ordinary(Arc<[u8]>),
    Leased(Lease),
}
impl From<Arc<[u8]>> for Backing {
    fn from(bytes: Arc<[u8]>) -> Self {
        Self::Ordinary(bytes)
    }
}
impl Backing {
    pub(in crate::interpreter) fn ordinary(&self) -> &Arc<[u8]> {
        match self {
            Self::Ordinary(bytes) => bytes,
            Self::Leased(_) => panic!("private leased backing reached an ordinary adapter"),
        }
    }
    pub(in crate::interpreter) fn ordinary_mut(&mut self) -> &mut Arc<[u8]> {
        match self {
            Self::Ordinary(bytes) => bytes,
            Self::Leased(_) => panic!("private leased backing reached an ordinary store"),
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
#[derive(Clone, Debug, PartialEq)]
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
        }
    }
    pub(in crate::interpreter) fn bytes(&self) -> &[u8] {
        assert!(
            self.lease.is_none(),
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
        }
    }
    pub(in crate::interpreter) fn read<T>(
        &self,
        registry: &mut Registry<'_>,
        consumer: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, Flow> {
        match self.lease {
            Some(view) => registry.read(view, self.start, self.end, consumer),
            None => Ok(consumer(self.bytes())),
        }
    }
}
impl OwnedBytesValue {
    pub(in crate::interpreter) fn read<T>(
        &self,
        registry: &mut Registry<'_>,
        consumer: impl FnOnce(&[u8]) -> T,
    ) -> Result<T, Flow> {
        match &self.bytes {
            Backing::Ordinary(bytes) => Ok(consumer(bytes)),
            Backing::Leased(lease) => registry.read_owner(lease, consumer),
        }
    }
}
