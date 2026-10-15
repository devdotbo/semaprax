//! Borrowed exact-current nominal type inventory for type reachability.
//!
//! The index points into declarations the caller already owns for the whole
//! closure walk. It is a lookup view, not proof or admission authority: every
//! profile check still runs over the borrowed declaration, and only the
//! selected declarations are cloned, once, at the owned publication boundary.

use std::collections::BTreeMap;

use super::{DeclarationId, ResolvedTypeDeclaration};

pub(crate) struct BorrowedTypeInventory<'a> {
    declarations: BTreeMap<&'a DeclarationId, &'a ResolvedTypeDeclaration>,
}

/// One identity occurred twice in the complete supplied inventory. Callers map
/// this to their own stable diagnostic text.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct DuplicateTypeIdentity;

impl<'a> BorrowedTypeInventory<'a> {
    /// Index every supplied declaration, reachable or not, refusing the first
    /// repeated identity before any closure walk exactly like the owned map.
    pub(crate) fn collect(
        declarations: impl IntoIterator<Item = &'a ResolvedTypeDeclaration>,
    ) -> Result<Self, DuplicateTypeIdentity> {
        let mut indexed = BTreeMap::new();
        for declaration in declarations {
            if indexed.insert(&declaration.id, declaration).is_some() {
                return Err(DuplicateTypeIdentity);
            }
        }
        Ok(Self {
            declarations: indexed,
        })
    }

    pub(crate) fn get(&self, id: &DeclarationId) -> Option<&'a ResolvedTypeDeclaration> {
        self.declarations.get(id).copied()
    }

    #[cfg(test)]
    pub(crate) fn len(&self) -> usize {
        self.declarations.len()
    }
}
