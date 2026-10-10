//! Borrowed exact-key census: no opaque tree allocation or identity clone.
use crate::hir::{ExpressionId, ResolvedBinding};
type Entry<'a> = (&'a ExpressionId, &'a ResolvedBinding);
#[derive(Default)]
pub(crate) struct HelperBindings<'a> {
    entries: Vec<Entry<'a>>,
}
impl<'a> HelperBindings<'a> {
    pub(crate) fn insert(
        &mut self,
        at: &'a ExpressionId,
        binding: &'a ResolvedBinding,
    ) -> Option<()> {
        let index = match self.entries.binary_search_by(|entry| entry.0.cmp(at)) {
            Ok(index) => {
                self.entries[index].1 = binding;
                return Some(());
            }
            Err(index) => index,
        };
        if self.entries.len() == self.entries.capacity() {
            let capacity = self.entries.capacity().checked_mul(2)?.max(1);
            let bytes = capacity.checked_mul(std::mem::size_of::<Entry<'a>>())?;
            let charge = if self.entries.capacity() == 0 {
                bytes.checked_add(std::mem::size_of::<Self>())?
            } else {
                bytes
            };
            // Both old and replacement allocations remain charged at the peak.
            // Global Vec::try_reserve_exact requests this exact backing layout;
            // no amortized growth, key backing or tree-node estimate is used.
            if !crate::bounded_output::reserve_active_required(charge) {
                return None;
            }
            let mut replacement = Vec::new();
            replacement.try_reserve_exact(capacity).ok()?;
            if replacement.capacity() != capacity {
                return None;
            }
            replacement.extend_from_slice(&self.entries);
            self.entries = replacement;
        }
        self.entries.insert(index, (at, binding));
        Some(())
    }
    #[cfg(test)]
    pub(crate) fn backing_capacity(&self) -> usize {
        self.entries.capacity()
    }
    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
    pub(crate) fn contains_key(&self, at: &ExpressionId) -> bool {
        self.get(at).is_some()
    }
    pub(crate) fn get(&self, at: &ExpressionId) -> Option<&'a ResolvedBinding> {
        self.entries
            .binary_search_by(|entry| entry.0.cmp(at))
            .ok()
            .map(|i| self.entries[i].1)
    }
    pub(crate) fn remove(&mut self, at: &ExpressionId) -> Option<&'a ResolvedBinding> {
        let i = self
            .entries
            .binary_search_by(|entry| entry.0.cmp(at))
            .ok()?;
        Some(self.entries.remove(i).1)
    }
}
impl<'a> IntoIterator for HelperBindings<'a> {
    type Item = Entry<'a>;
    type IntoIter = std::vec::IntoIter<Entry<'a>>;
    fn into_iter(self) -> Self::IntoIter {
        self.entries.into_iter()
    }
}
