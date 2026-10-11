//! Borrowed numeric discovery only. Exact identity is checked by each receiver.
use super::{Diagnostic, resolved};
use crate::hir::{ExpressionId, ResolvedBinding};
#[derive(Debug)]
pub(super) struct Site<'a> {
    pub(super) at: &'a ExpressionId,
    pub(super) binding: &'a ResolvedBinding,
}
pub(super) fn charge(work: &mut usize) -> Result<(), Diagnostic> {
    *work = work.checked_sub(1).ok_or_else(resolved::census_error)?;
    Ok(())
}
pub(super) fn reserve<T>(count: usize) -> Result<Vec<T>, Diagnostic> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(resolved::census_error)?;
    if !crate::bounded_output::reserve_active_required(bytes) {
        return Err(resolved::census_error());
    }
    let mut entries = Vec::new();
    entries
        .try_reserve_exact(count)
        .map_err(|_| resolved::census_error())?;
    if entries.capacity() != count {
        return Err(resolved::census_error());
    }
    Ok(entries)
}
// In-place heapsort has no scratch allocation. Every actual comparison and
// relocation is charged before evaluation; exhaustion never runs an uncharged
// remainder or publishes a partial directory.
pub(super) fn sort<T>(
    entries: &mut [T],
    key: impl Fn(&T) -> usize,
    work: &mut usize,
) -> Result<(), Diagnostic> {
    fn sift<T>(
        entries: &mut [T],
        mut root: usize,
        end: usize,
        key: &impl Fn(&T) -> usize,
        work: &mut usize,
    ) -> Result<(), Diagnostic> {
        while let Some(left) = root
            .checked_mul(2)
            .and_then(|n| n.checked_add(1))
            .filter(|n| *n < end)
        {
            let mut child = left;
            if left + 1 < end {
                charge(work)?;
                if key(&entries[left]) < key(&entries[left + 1]) {
                    child = left + 1;
                }
            }
            charge(work)?;
            if key(&entries[root]) >= key(&entries[child]) {
                break;
            }
            charge(work)?;
            entries.swap(root, child);
            root = child;
        }
        Ok(())
    }
    let length = entries.len();
    for root in (0..length / 2).rev() {
        sift(entries, root, length, &key, work)?;
    }
    for end in (1..entries.len()).rev() {
        charge(work)?;
        entries.swap(0, end);
        sift(entries, 0, end, &key, work)?;
    }
    Ok(())
}
pub(super) fn lower<T>(
    entries: &[T],
    wanted: usize,
    key: impl Fn(&T) -> usize,
    mut probe: impl FnMut(),
) -> usize {
    let (mut start, mut end) = (0, entries.len());
    while start < end {
        let middle = start + (end - start) / 2;
        probe();
        if key(&entries[middle]) < wanted {
            start = middle + 1;
        } else {
            end = middle;
        }
    }
    start
}
