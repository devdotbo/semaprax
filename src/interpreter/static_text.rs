//! Runtime literal views retain the authenticated module backing without allocation.
use super::*;
use std::ops::Deref;

#[derive(Clone, Debug, PartialEq)]
pub(super) enum BorrowedRoot {
    Invocation(ValueId),
    SourceExprStatic(hir::ExpressionId),
    VectorStringSlot {
        root: ValueId,
        producer: hir::ExpressionId,
        index: usize,
        generation: u64,
    },
}
impl From<ValueId> for BorrowedRoot {
    fn from(value: ValueId) -> Self {
        Self::Invocation(value)
    }
}
#[derive(Clone, Debug, PartialEq)]
pub(super) enum Utf8Backing {
    Checked(checked_allocation::Backing),
    Invocation(Arc<[u8]>),
    Static(hir::StaticUtf8),
    VectorStringSlot {
        vector: Arc<owned_vec::OwnedVecValue>,
        index: usize,
        generation: u64,
    },
}
impl From<Arc<[u8]>> for Utf8Backing {
    fn from(value: Arc<[u8]>) -> Self {
        Self::Invocation(value)
    }
}
impl AsRef<[u8]> for Utf8Backing {
    fn as_ref(&self) -> &[u8] {
        match self {
            Self::Invocation(bytes) => bytes,
            Self::Checked(backing) => backing.as_ref(),
            Self::Static(text) => text.as_str().as_bytes(),
            Self::VectorStringSlot {
                vector,
                index,
                generation,
            } => {
                assert_eq!(
                    vector.generation, *generation,
                    "String-slot generation changed"
                );
                let Value::String(text) = &vector.values[*index] else {
                    unreachable!("authenticated String-slot backing changed type")
                };
                text.as_bytes()
            }
        }
    }
}
impl Deref for Utf8Backing {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        self.as_ref()
    }
}
