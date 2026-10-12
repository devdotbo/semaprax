//! Paid persistent symbolic binding environment. Types borrow only the exact
//! current immutable template; checkpoints grant no validation authority.
use super::*;
use std::borrow::Cow;
mod charge;
mod tree;

pub(in crate::hir::validation) enum Values<'value> {
    Borrowed(tree::Root<'value>),
    #[cfg(test)]
    LegacyBorrowed(BTreeMap<ValueId, &'value ResolvedType>),
    #[cfg(test)]
    Owned(BTreeMap<ValueId, ResolvedType>),
}
impl<'value> Values<'value> {
    pub(in crate::hir::validation) fn new() -> Result<Self, Diagnostic> {
        charge::control()?;
        Ok(Self::Borrowed(None))
    }
    pub(in crate::hir::validation) fn checkpoint(&self) -> Result<Self, Diagnostic> {
        match self {
            Self::Borrowed(root) => {
                charge::control()?;
                Ok(Self::Borrowed(root.clone()))
            }
            #[cfg(test)]
            Self::LegacyBorrowed(values) => Ok(Self::LegacyBorrowed(values.clone())),
            // Complete original map snapshot, including every owned type.
            #[cfg(test)]
            Self::Owned(values) => Ok(Self::Owned(values.clone())),
        }
    }
    pub(in crate::hir::validation) fn empty_like(&self) -> Result<Self, Diagnostic> {
        match self {
            Self::Borrowed(_) => Self::new(),
            #[cfg(test)]
            Self::LegacyBorrowed(_) => Ok(Self::borrowed_reference()),
            #[cfg(test)]
            Self::Owned(_) => Ok(Self::owned()),
        }
    }
    pub(in crate::hir::validation) fn insert(
        &mut self,
        id: ValueId,
        ty: &'value ResolvedType,
    ) -> Result<bool, Diagnostic> {
        match self {
            Self::Borrowed(root) => {
                charge::active()?;
                let replaced = tree::get(root.as_deref(), &id).is_some();
                // Never take the published root before all changed-path and
                // rotation allocations succeed. Failure leaves it complete.
                tree::insert_current(root, id, ty)?;
                Ok(replaced)
            }
            #[cfg(test)]
            Self::LegacyBorrowed(values) => Ok(values.insert(id, ty).is_some()),
            #[cfg(test)]
            Self::Owned(values) => Ok(values.insert(id, ty.clone()).is_some()),
        }
    }
    pub(in crate::hir::validation) fn get(&self, id: &ValueId) -> Option<&ResolvedType> {
        match self {
            Self::Borrowed(root) => tree::get(root.as_deref(), id),
            #[cfg(test)]
            Self::LegacyBorrowed(values) => values.get(id).copied(),
            #[cfg(test)]
            Self::Owned(values) => values.get(id),
        }
    }
    pub(in crate::hir::validation) fn root(&self, id: &ValueId) -> Option<Cow<'_, ResolvedType>> {
        match self {
            Self::Borrowed(root) => tree::get(root.as_deref(), id).map(Cow::Borrowed),
            #[cfg(test)]
            Self::LegacyBorrowed(values) => values.get(id).map(|ty| Cow::Borrowed(*ty)),
            #[cfg(test)]
            Self::Owned(values) => values.get(id).map(|ty| Cow::Owned(ty.clone())),
        }
    }
    #[cfg(test)]
    fn borrowed_reference() -> Self {
        Self::LegacyBorrowed(BTreeMap::new())
    }
    #[cfg(test)]
    fn owned() -> Self {
        Self::Owned(BTreeMap::new())
    }
    // Full observation only; callers construct this projection outside the
    // allocator window. It is not retained or consumed by production admission.
    #[cfg(test)]
    fn ordered_entries(&self) -> Vec<(ValueId, ResolvedType)> {
        match self {
            Self::Borrowed(root) => {
                let mut result = Vec::new();
                tree::ordered_entries(root.as_deref(), &mut result);
                result
            }
            Self::LegacyBorrowed(values) => values
                .iter()
                .map(|(id, ty)| (id.clone(), (*ty).clone()))
                .collect(),
            Self::Owned(values) => values
                .iter()
                .map(|(id, ty)| (id.clone(), ty.clone()))
                .collect(),
        }
    }
}
#[cfg(test)]
mod tests;

#[cfg(test)]
mod checkpoint_tests;
