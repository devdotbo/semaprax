//! Constructor-only ID discovery, retaining the first original ordinal.
//! Source spelling discovers a current pointer; complete target proof still
//! supplies authority. No negative or transitive proof is cached here.
use super::{Diagnostic, ResolvedFunction, directory, resolved};
use std::cmp::Ordering;
#[derive(Debug)]
pub(super) struct Functions<'a> {
    source: &'a [ResolvedFunction],
    entries: Vec<usize>,
}
pub(super) fn compare(left: &str, right: &str, work: &mut usize) -> Result<Ordering, Diagnostic> {
    for (a, b) in left.bytes().zip(right.bytes()) {
        directory::charge(work)?;
        let order = a.cmp(&b);
        if order != Ordering::Equal {
            return Ok(order);
        }
    }
    directory::charge(work)?;
    Ok(left.len().cmp(&right.len()))
}
impl<'a> Functions<'a> {
    pub(super) fn new(
        source: &'a [ResolvedFunction],
        work: &mut usize,
    ) -> Result<Self, Diagnostic> {
        if !crate::bounded_output::reserve_active_required(
            std::mem::size_of::<Self>() + std::mem::size_of::<Vec<usize>>(),
        ) {
            return Err(resolved::census_error());
        }
        let mut entries = directory::reserve::<usize>(source.len())?;
        let mut scratch = directory::reserve::<usize>(source.len())?;
        for ordinal in 0..source.len() {
            directory::units(work, 2)?;
            entries.push(ordinal);
            scratch.push(ordinal);
        }
        let mut width = 1usize;
        while width < source.len() {
            let step = width.checked_mul(2).ok_or_else(resolved::census_error)?;
            for start in (0..source.len()).step_by(step) {
                let middle = start
                    .checked_add(width)
                    .ok_or_else(resolved::census_error)?
                    .min(source.len());
                let end = start
                    .checked_add(step)
                    .ok_or_else(resolved::census_error)?
                    .min(source.len());
                let (mut left, mut right) = (start, middle);
                for destination in &mut scratch[start..end] {
                    let take_left = if right == end {
                        true
                    } else if left == middle {
                        false
                    } else {
                        compare(
                            source[entries[left]].id.as_str(),
                            source[entries[right]].id.as_str(),
                            work,
                        )? != Ordering::Greater
                    };
                    directory::charge(work)?;
                    *destination = if take_left {
                        let index = entries[left];
                        left += 1;
                        index
                    } else {
                        let index = entries[right];
                        right += 1;
                        index
                    };
                }
            }
            directory::units(work, entries.len())?;
            entries.copy_from_slice(&scratch);
            width = step;
        }
        Ok(Self { source, entries })
    }
    pub(super) fn get(&self, wanted: &str, work: &mut usize) -> Result<Option<usize>, Diagnostic> {
        let (mut start, mut end) = (0, self.entries.len());
        while start < end {
            let middle = start + (end - start) / 2;
            if compare(self.source[self.entries[middle]].id.as_str(), wanted, work)?
                == Ordering::Less
            {
                start = middle + 1;
            } else {
                end = middle;
            }
        }
        let Some(&ordinal) = self.entries.get(start) else {
            return Ok(None);
        };
        Ok(
            (compare(self.source[ordinal].id.as_str(), wanted, work)? == Ordering::Equal)
                .then_some(ordinal),
        )
    }
}
