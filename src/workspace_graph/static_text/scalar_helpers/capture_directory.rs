//! Invocation-held function lookup scratch. It does not authenticate signatures,
//! origins, source bodies or namespace ownership; the capture caller does all of
//! those unchanged comparisons after every lookup.
use super::{Program, capacity, fail, hir, pure, reserve_builder_structure};
use crate::diagnostic::Diagnostic;
use std::cmp::Ordering;

struct Index<'a> {
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
    reserve_builder_structure(bytes)
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
            debit(work, 1)?;
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
        debit(work, 1)?;
        targets.swap(root, greatest);
        root = greatest;
    }
    Ok(())
}

/// Borrowed facts only; source origin and complete signature authentication stay
/// in capture before any fact is published. Selected functions keep AST order.
pub(super) struct Directory<'a> {
    selected: Vec<Option<&'a hir::ResolvedFunction>>,
    count: usize,
}
impl<'a> Directory<'a> {
    pub(super) fn new(
        program: &Program,
        resolved: &'a hir::ResolvedProgram,
        work: &mut usize,
    ) -> Result<Self, Vec<Diagnostic>> {
        Self::classify(program, resolved, work, &mut pure)
    }
    fn classify(
        program: &Program,
        resolved: &'a hir::ResolvedProgram,
        work: &mut usize,
        classify: &mut dyn FnMut(
            &hir::ResolvedFunction,
            &mut usize,
        ) -> Result<bool, Vec<Diagnostic>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let result = Self::build(program, resolved, work, classify);
        if result.is_err() {
            *work = 0;
        }
        result
    }
    fn build(
        program: &Program,
        resolved: &'a hir::ResolvedProgram,
        work: &mut usize,
        classify: &mut dyn FnMut(
            &hir::ResolvedFunction,
            &mut usize,
        ) -> Result<bool, Vec<Diagnostic>>,
    ) -> Result<Self, Vec<Diagnostic>> {
        let index = Index::new(&resolved.functions, work)?;
        let bytes = program
            .functions
            .len()
            .checked_mul(std::mem::size_of::<Option<&hir::ResolvedFunction>>())
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>()))
            .ok_or_else(capacity)?;
        reserve(bytes)?;
        let mut selected = Vec::new();
        selected
            .try_reserve_exact(program.functions.len())
            .map_err(|_| capacity())?;
        if selected.capacity() != program.functions.len() {
            return Err(capacity());
        }
        let mut count = 0usize;
        for source in &program.functions {
            debit(work, source.params.len().checked_add(1).ok_or_else(fail)?)?;
            let eligible = source.explicit_id
                && source.type_parameters.is_empty()
                && source.follows.is_none()
                && source.params.iter().all(|p| {
                    matches!(
                        p.mode,
                        crate::ast::ParamMode::Value | crate::ast::ParamMode::Borrow
                    )
                });
            let current = if eligible {
                let function = index.find(&source.stable_id, work)?;
                if classify(function, work)? {
                    count = count.checked_add(1).ok_or_else(capacity)?;
                    Some(function)
                } else {
                    None
                }
            } else {
                None
            };
            selected.push(current);
        }
        Ok(Self { selected, count })
    }
    pub(super) fn count(&self) -> usize {
        self.count
    }
    pub(super) fn selected(&self, ordinal: usize) -> Option<&'a hir::ResolvedFunction> {
        self.selected.get(ordinal).copied().flatten()
    }
}
#[cfg(test)]
mod tests;
