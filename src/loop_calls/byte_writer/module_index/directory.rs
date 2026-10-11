//! Borrowed numeric discovery only. Exact identity is checked by each receiver.
use super::{resolved, Diagnostic};
use crate::hir::{ExpressionId, ResolvedBinding};
#[derive(Clone, Copy, Debug)]
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
// Numeric addresses influence discovery order, never acceptance work. Fixed
// byte passes visit every item and histogram cell regardless of address ranks.
// Both replacement backing and fixed histogram are charged before allocation.
pub(super) fn sort<T: Copy>(
    entries: &mut [T],
    key: impl Fn(&T) -> usize,
    work: &mut usize,
) -> Result<(), Diagnostic> {
    if entries.len() < 2 {
        return Ok(());
    }
    if !crate::bounded_output::reserve_active_required(
        std::mem::size_of::<[usize; 256]>() + std::mem::size_of::<Vec<T>>(),
    ) {
        return Err(resolved::census_error());
    }
    units(work, 256)?;
    let mut offsets = [0usize; 256];
    let mut scratch = reserve::<T>(entries.len())?;
    units(work, entries.len())?;
    scratch.extend_from_slice(entries);
    for byte in 0..std::mem::size_of::<usize>() {
        units(work, 256)?;
        offsets.fill(0);
        for item in entries.iter() {
            charge(work)?;
            let digit = (key(item) >> (byte * 8)) & 255;
            offsets[digit] = offsets[digit]
                .checked_add(1)
                .ok_or_else(resolved::census_error)?;
        }
        let mut start = 0usize;
        for offset in &mut offsets {
            charge(work)?;
            let length = *offset;
            *offset = start;
            start = start
                .checked_add(length)
                .ok_or_else(resolved::census_error)?;
        }
        for item in entries.iter() {
            charge(work)?;
            let digit = (key(item) >> (byte * 8)) & 255;
            scratch[offsets[digit]] = *item;
            offsets[digit] += 1;
        }
        units(work, entries.len())?;
        entries.copy_from_slice(&scratch);
    }
    Ok(())
}
pub(super) fn units(work: &mut usize, count: usize) -> Result<(), Diagnostic> {
    *work = work.checked_sub(count).ok_or_else(resolved::census_error)?;
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
