//! Invocation-held function lookup scratch. It does not authenticate signatures,
//! origins, source bodies or namespace ownership; the capture caller does all of
//! those unchanged comparisons after every lookup.
use super::{capacity, fail, hir};
use crate::diagnostic::Diagnostic;
use std::cmp::Ordering;

pub(super) struct Index<'a> {
    targets: Vec<&'a hir::ResolvedFunction>,
}
fn debit(work: &mut usize, units: usize) -> Result<(), Vec<Diagnostic>> {
    match work.checked_sub(units) {
        Some(remaining) => {
            *work = remaining;
            Ok(())
        }
        None => {
            *work = 0;
            Err(fail())
        }
    }
}
fn reserve(bytes: usize) -> Result<(), Vec<Diagnostic>> {
    if crate::bounded_output::active_refused()
        || !crate::bounded_output::reserve_active_required(bytes)
    {
        return Err(capacity());
    }
    Ok(())
}
fn compare(a: &str, b: &str, work: &mut usize) -> Result<Ordering, Vec<Diagnostic>> {
    debit(work, a.len().min(b.len()).checked_add(1).ok_or_else(fail)?)?;
    Ok(a.cmp(b))
}
impl<'a> Index<'a> {
    pub(super) fn new(
        functions: &'a [hir::ResolvedFunction],
        work: &mut usize,
    ) -> Result<Self, Vec<Diagnostic>> {
        debit(work, functions.len())?;
        let bytes = functions
            .len()
            .checked_mul(std::mem::size_of::<&hir::ResolvedFunction>())
            .ok_or_else(capacity)?;
        reserve(std::mem::size_of::<Self>())?;
        reserve(bytes)?;
        let mut targets = Vec::new();
        targets
            .try_reserve_exact(functions.len())
            .map_err(|_| capacity())?;
        // No extra capacity becomes an admission after allocation. This carrier
        // makes one fixed reservation, never grows, and owns references only.
        if targets.capacity() != functions.len() {
            return Err(capacity());
        }
        targets.extend(functions);
        for root in (0..targets.len() / 2).rev() {
            sift(&mut targets, root, functions.len(), work)?;
        }
        for end in (1..targets.len()).rev() {
            targets.swap(0, end);
            sift(&mut targets, 0, end, work)?;
        }
        for pair in targets.windows(2) {
            if compare(pair[0].id.as_str(), pair[1].id.as_str(), work)? == Ordering::Equal {
                return Err(fail());
            }
        }
        Ok(Self { targets })
    }
    pub(super) fn find(
        &self,
        id: &str,
        work: &mut usize,
    ) -> Result<&'a hir::ResolvedFunction, Vec<Diagnostic>> {
        let (mut low, mut high) = (0, self.targets.len());
        while low < high {
            debit(work, 1)?;
            let middle = low + (high - low) / 2;
            match compare(self.targets[middle].id.as_str(), id, work)? {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(self.targets[middle]),
            }
        }
        Err(fail())
    }
}
fn sift(
    targets: &mut [&hir::ResolvedFunction],
    mut root: usize,
    end: usize,
    work: &mut usize,
) -> Result<(), Vec<Diagnostic>> {
    while let Some(child) = root
        .checked_mul(2)
        .and_then(|n| n.checked_add(1))
        .filter(|n| *n < end)
    {
        debit(work, 1)?;
        let mut greatest = child;
        if child + 1 < end
            && compare(
                targets[child].id.as_str(),
                targets[child + 1].id.as_str(),
                work,
            )? == Ordering::Less
        {
            greatest += 1;
        }
        if compare(
            targets[root].id.as_str(),
            targets[greatest].id.as_str(),
            work,
        )? != Ordering::Less
        {
            break;
        }
        targets.swap(root, greatest);
        root = greatest;
    }
    Ok(())
}
pub(super) fn lookup<'a>(
    index: Option<&Index<'a>>,
    functions: &'a [hir::ResolvedFunction],
    id: &str,
    work: &mut usize,
) -> Result<&'a hir::ResolvedFunction, Vec<Diagnostic>> {
    if let Some(index) = index {
        return index.find(id, work);
    }
    #[cfg(test)]
    {
        legacy(functions, id, work)
    }
    #[cfg(not(test))]
    {
        let _ = (functions, id, work);
        Err(fail())
    }
}
// Independent original full scan: test-only, with the original per-inventory
// debit and exact duplicate/missing refusal. No production fallback scan.
#[cfg(test)]
fn legacy<'a>(
    functions: &'a [hir::ResolvedFunction],
    id: &str,
    work: &mut usize,
) -> Result<&'a hir::ResolvedFunction, Vec<Diagnostic>> {
    debit(work, functions.len())?;
    let mut matches = functions.iter().filter(|f| f.id.as_str() == id);
    let result = matches.next().ok_or_else(fail)?;
    if matches.next().is_some() {
        return Err(fail());
    }
    Ok(result)
}
#[cfg(test)]
mod tests;
