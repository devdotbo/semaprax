//! Allocation-free ordering whose actual comparisons stop at work refusal.
use super::{refusal, shape::debit};
use crate::diagnostic::Diagnostic;
use std::cmp::Ordering;

pub(super) fn identity(
    left: &str,
    right: &str,
    work: &mut usize,
) -> Result<Ordering, Vec<Diagnostic>> {
    debit(
        work,
        left.len()
            .checked_add(right.len())
            .and_then(|n| n.checked_add(1))
            .ok_or_else(refusal)?,
    )?;
    Ok(left.cmp(right))
}

pub(super) fn sort<T>(
    values: &mut [T],
    work: &mut usize,
    mut compare: impl FnMut(&T, &T, &mut usize) -> Result<Ordering, Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    fn sift<T>(
        values: &mut [T],
        mut root: usize,
        end: usize,
        work: &mut usize,
        compare: &mut impl FnMut(&T, &T, &mut usize) -> Result<Ordering, Vec<Diagnostic>>,
    ) -> Result<(), Vec<Diagnostic>> {
        loop {
            debit(work, 1)?;
            let Some(left) = root.checked_mul(2).and_then(|n| n.checked_add(1)) else {
                return Err(refusal());
            };
            if left >= end {
                return Ok(());
            }
            let mut child = left;
            if left + 1 < end && compare(&values[left], &values[left + 1], work)? == Ordering::Less
            {
                child += 1;
            }
            if compare(&values[root], &values[child], work)? != Ordering::Less {
                return Ok(());
            }
            debit(work, 1)?;
            values.swap(root, child);
            root = child;
        }
    }
    for root in (0..values.len() / 2).rev() {
        sift(values, root, values.len(), work, &mut compare)?;
    }
    for end in (1..values.len()).rev() {
        debit(work, 1)?;
        values.swap(0, end);
        sift(values, 0, end, work, &mut compare)?;
    }
    Ok(())
}
