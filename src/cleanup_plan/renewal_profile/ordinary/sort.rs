//! In-place heap sort: no opaque sorting allocation and no activity after refusal.
use super::{Admission, Entry, Refusal};
use crate::hir::ExpressionId;
use std::cmp::Ordering;
pub(super) fn compare_keys(
    left: &ExpressionId,
    right: &ExpressionId,
    admission: &mut Admission<'_>,
) -> Result<Ordering, Refusal> {
    let bytes =
        admission.arithmetic(left.as_str().len().min(right.as_str().len()).checked_add(1))?;
    admission.units(bytes)?;
    Ok(left.cmp(right))
}
fn compare(
    left: Entry<'_>,
    right: Entry<'_>,
    admission: &mut Admission<'_>,
) -> Result<Ordering, Refusal> {
    let order = compare_keys(left.at, right.at, admission)?;
    admission.units(4)?;
    Ok(order
        .then((left.family as u8).cmp(&(right.family as u8)))
        .then(left.rank.cmp(&right.rank))
        .then(if left.first && right.first {
            right.ordinal.cmp(&left.ordinal)
        } else {
            left.ordinal.cmp(&right.ordinal)
        }))
}
fn sift(
    entries: &mut [Entry<'_>],
    mut root: usize,
    admission: &mut Admission<'_>,
) -> Result<(), Refusal> {
    loop {
        admission.units(1)?;
        let Some(child) = root.checked_mul(2).and_then(|n| n.checked_add(1)) else {
            return admission.refuse(Refusal::Arithmetic);
        };
        if child >= entries.len() {
            return Ok(());
        }
        let mut greatest = child;
        if child + 1 < entries.len()
            && compare(entries[child], entries[child + 1], admission)?.is_lt()
        {
            greatest += 1;
        }
        if !compare(entries[root], entries[greatest], admission)?.is_lt() {
            return Ok(());
        }
        admission.units(1)?;
        entries.swap(root, greatest);
        root = greatest;
    }
}
pub(super) fn sort(
    entries: &mut [Entry<'_>],
    admission: &mut Admission<'_>,
) -> Result<(), Refusal> {
    for root in (0..entries.len() / 2).rev() {
        sift(entries, root, admission)?;
    }
    for end in (1..entries.len()).rev() {
        admission.units(1)?;
        entries.swap(0, end);
        sift(&mut entries[..end], 0, admission)?;
    }
    Ok(())
}

// Collector scratch uses the same no-allocation algorithm over borrowed keys.
pub(crate) fn heap<T>(
    entries: &mut [T],
    admission: &mut Admission<'_>,
    mut compare: impl FnMut(&T, &T, &mut Admission<'_>) -> Result<Ordering, Refusal>,
) -> Result<(), Refusal> {
    fn sift<T>(
        entries: &mut [T],
        mut root: usize,
        admission: &mut Admission<'_>,
        compare: &mut impl FnMut(&T, &T, &mut Admission<'_>) -> Result<Ordering, Refusal>,
    ) -> Result<(), Refusal> {
        loop {
            admission.units(1)?;
            let child = admission.arithmetic(root.checked_mul(2).and_then(|n| n.checked_add(1)))?;
            if child >= entries.len() {
                return Ok(());
            }
            let mut greatest = child;
            if child + 1 < entries.len()
                && compare(&entries[child], &entries[child + 1], admission)?.is_lt()
            {
                greatest += 1;
            }
            if !compare(&entries[root], &entries[greatest], admission)?.is_lt() {
                return Ok(());
            }
            admission.units(1)?;
            entries.swap(root, greatest);
            root = greatest;
        }
    }
    for root in (0..entries.len() / 2).rev() {
        sift(entries, root, admission, &mut compare)?;
    }
    for end in (1..entries.len()).rev() {
        admission.units(1)?;
        entries.swap(0, end);
        sift(&mut entries[..end], 0, admission, &mut compare)?;
    }
    Ok(())
}
