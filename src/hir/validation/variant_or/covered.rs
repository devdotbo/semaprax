//! Sorted borrowed case identities; only the actual reference backing is owned.
//! Reuse the validator's prepaid continuation storage for growth and snapshots.
use super::*;

pub(in crate::hir::validation) struct Set<'a> {
    values: scope_checkpoint::Storage<&'a DeclarationId>,
}
// The owning carrier must fit the complete old frame's existing BTreeSet slot.
const _: () =
    assert!(std::mem::size_of::<Set<'static>>() == std::mem::size_of::<BTreeSet<DeclarationId>>());
impl<'a> Set<'a> {
    pub(in crate::hir::validation) fn new() -> Self {
        Self {
            values: scope_checkpoint::Storage::new(),
        }
    }
    fn position(&self, key: &DeclarationId) -> Result<usize, usize> {
        let (mut low, mut high) = (0, self.values.len());
        while low < high {
            let middle = low + (high - low) / 2;
            match self.values[middle].cmp(key) {
                std::cmp::Ordering::Less => low = middle + 1,
                std::cmp::Ordering::Greater => high = middle,
                std::cmp::Ordering::Equal => return Ok(middle),
            }
        }
        Err(low)
    }
    pub(in crate::hir::validation) fn contains(&self, key: &DeclarationId) -> bool {
        self.position(key).is_ok()
    }
    pub(in crate::hir::validation) fn len(&self) -> usize {
        self.values.len()
    }
    pub(in crate::hir::validation) fn insert(
        &mut self,
        key: &'a DeclarationId,
    ) -> Result<bool, Diagnostic> {
        scope_checkpoint::charge::reserve(Some(0))?;
        let Err(at) = self.position(key) else {
            return Ok(false);
        };
        let len = self.values.len();
        // The paid owner reserves any full replacement before prior values move.
        self.values.push(key)?;
        for index in (at..len).rev() {
            self.values[index + 1] = self.values[index];
        }
        self.values[at] = key;
        Ok(true)
    }
    pub(in crate::hir::validation) fn checkpoint(&self) -> Result<Self, Diagnostic> {
        scope_checkpoint::charge::reserve(Some(0))?;
        let mut values = scope_checkpoint::Storage::with_capacity(self.values.len())?;
        for index in 0..self.values.len() {
            values.push(self.values[index])?;
        }
        Ok(Self { values })
    }
    #[cfg(test)]
    pub(in crate::hir::validation) fn owned_capacity(&self) -> usize {
        self.values.capacity() * std::mem::size_of::<&DeclarationId>()
    }
    #[cfg(test)]
    fn iter(&self) -> impl Iterator<Item = &'a DeclarationId> + '_ {
        self.values.iter().copied()
    }
}
#[cfg(test)]
mod tests;
