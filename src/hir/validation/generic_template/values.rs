//! Exact symbolic binding environment. Snapshot node order stays unchanged;
//! immutable types remain held by the current template, never by this directory.
use super::*;
use std::borrow::Cow;

#[derive(Clone)]
pub(in crate::hir::validation) enum Values<'value> {
    Borrowed(BTreeMap<ValueId, &'value ResolvedType>),
    #[cfg(test)]
    Owned(BTreeMap<ValueId, ResolvedType>),
}
impl<'value> Values<'value> {
    pub(in crate::hir::validation) fn new() -> Self {
        Self::Borrowed(BTreeMap::new())
    }
    pub(in crate::hir::validation) fn empty_like(&self) -> Self {
        match self {
            Self::Borrowed(_) => Self::new(),
            #[cfg(test)]
            Self::Owned(_) => Self::owned(),
        }
    }
    pub(in crate::hir::validation) fn insert(
        &mut self,
        id: ValueId,
        ty: &'value ResolvedType,
    ) -> bool {
        match self {
            Self::Borrowed(values) => values.insert(id, ty).is_some(),
            #[cfg(test)]
            Self::Owned(values) => values.insert(id, ty.clone()).is_some(),
        }
    }
    pub(in crate::hir::validation) fn get(&self, id: &ValueId) -> Option<&ResolvedType> {
        match self {
            Self::Borrowed(values) => values.get(id).copied(),
            #[cfg(test)]
            Self::Owned(values) => values.get(id),
        }
    }
    pub(in crate::hir::validation) fn root(&self, id: &ValueId) -> Option<Cow<'_, ResolvedType>> {
        match self {
            Self::Borrowed(values) => values.get(id).map(|ty| Cow::Borrowed(*ty)),
            #[cfg(test)]
            Self::Owned(values) => values.get(id).map(|ty| Cow::Owned(ty.clone())),
        }
    }
    #[cfg(test)]
    fn owned() -> Self {
        Self::Owned(BTreeMap::new())
    }
}
#[cfg(test)]
mod tests;
