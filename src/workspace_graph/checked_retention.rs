//! Exact contiguous views of immutable checked HIR; no validation authority.
use super::*;
use crate::project::incremental::checked_hir::CheckedModuleLease;
#[cfg(test)]
use std::ops::DerefMut;
use std::ops::{Deref, Range};
mod retain;
pub(super) use retain::retain_checked_module;

pub(super) enum Retained<T> {
    Owned(Vec<T>),
    Checked {
        owner: CheckedModuleLease,
        range: Range<usize>,
        read: fn(&hir::ResolvedProgram) -> &[T],
    },
}
impl<T> From<Vec<T>> for Retained<T> {
    fn from(items: Vec<T>) -> Self {
        Self::Owned(items)
    }
}
impl<T> Deref for Retained<T> {
    type Target = [T];
    fn deref(&self) -> &[T] {
        match self {
            Self::Owned(items) => items,
            Self::Checked { owner, range, read } => &read(owner)[range.clone()],
        }
    }
}
impl<T: Clone> Retained<T> {
    fn select(
        owner: CheckedModuleLease,
        read: fn(&hir::ResolvedProgram) -> &[T],
        keep: impl Fn(&T) -> bool,
    ) -> Result<Self, Vec<Diagnostic>> {
        let items = read(&owner);
        let mut start = None;
        let mut end = 0;
        let mut gap = false;
        let mut contiguous = true;
        for (index, item) in items.iter().enumerate() {
            if keep(item) {
                if gap {
                    contiguous = false;
                }
                start.get_or_insert(index);
                end = index + 1;
            } else if start.is_some() {
                gap = true;
            }
        }
        if contiguous {
            // Range and owner handles are inline, covered by the actual module
            // carrier. Arc clones allocate no extra control block.
            Ok(Self::Checked {
                owner,
                range: start.unwrap_or(0)..end,
                read,
            })
        } else {
            // Truthful compatibility fallback: the unchanged cached two-copy
            // prebound still funds the copied HIR. The actual output carrier is
            // reserved before allocation, as with ordinary retained filtering.
            let count = items.iter().filter(|item| keep(item)).count();
            let reserved = count
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
            reserve_builder_structure(reserved)?;
            let mut output = Vec::with_capacity(count);
            let actual = output
                .capacity()
                .checked_mul(std::mem::size_of::<T>())
                .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
            if actual > reserved {
                reserve_builder_structure(actual - reserved)?;
            }
            for item in items.iter().filter(|item| keep(item)) {
                output.push(item.clone());
            }
            Ok(Self::Owned(output))
        }
    }
    pub(super) fn into_owned(self) -> Vec<T> {
        match self {
            Self::Owned(items) => items,
            Self::Checked { owner, range, read } => read(&owner)[range].to_vec(),
        }
    }
    #[cfg(test)]
    pub(super) fn push(&mut self, item: T) {
        self.materialize().push(item);
    }
    #[cfg(test)]
    fn materialize(&mut self) -> &mut Vec<T> {
        if let Self::Checked { .. } = self {
            *self = Self::Owned(self.to_vec());
        }
        let Self::Owned(items) = self else {
            unreachable!()
        };
        items
    }
}
#[cfg(test)]
impl<T: Clone> DerefMut for Retained<T> {
    fn deref_mut(&mut self) -> &mut [T] {
        self.materialize()
    }
}
impl<'a, T> IntoIterator for &'a Retained<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}
impl<T: Clone> IntoIterator for Retained<T> {
    type Item = T;
    type IntoIter = std::vec::IntoIter<T>;
    fn into_iter(self) -> Self::IntoIter {
        self.into_owned().into_iter()
    }
}

#[cfg(test)]
impl<'a, T: Clone> IntoIterator for &'a mut Retained<T> {
    type Item = &'a mut T;
    type IntoIter = std::slice::IterMut<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.materialize().iter_mut()
    }
}
impl<T: std::fmt::Debug> std::fmt::Debug for Retained<T> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.deref().fmt(f)
    }
}

#[cfg(test)]
mod tests;

// Preserve the legacy full-program carrier price and pay any actual enum tag
// growth. The new representation is not authority to reduce the old forecast.
pub(super) fn reserve_input_carriers(count: usize) -> Result<(), Vec<Diagnostic>> {
    let old = std::mem::size_of::<(String, hir::ResolvedProgram)>();
    let new = std::mem::size_of::<(String, ResolvedModuleInput)>();
    let each = std::mem::size_of::<String>()
        .checked_add(GRAPH_ACCOUNTED_RESOLVED_PROGRAM_BYTES)
        .and_then(|bytes| bytes.checked_add(new.saturating_sub(old)))
        .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?;
    reserve_builder_structure(
        count
            .checked_mul(each)
            .ok_or_else(|| vec![limit_error("builder_bytes", active_builder_limit())])?,
    )
}
