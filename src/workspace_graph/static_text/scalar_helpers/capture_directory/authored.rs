//! Invocation-held borrowed view of the authored identity map. A hit grants no
//! origin authority: capture still compares namespace, path, explicit identity,
//! AST pointer and the complete signature after every lookup, exactly as it did
//! after the former linear scan.
use super::{capacity, compare, debit, fail, reserve};
use crate::diagnostic::Diagnostic;
use crate::workspace_graph::AuthoredDeclaration;
use std::cmp::Ordering;
use std::collections::BTreeMap;

type Entry<'m, 'a> = (&'m str, &'m AuthoredDeclaration<'a>);

pub(in super::super) struct Authored<'m, 'a> {
    entries: Vec<Entry<'m, 'a>>,
}
impl<'m, 'a> Authored<'m, 'a> {
    pub(in super::super) fn new<'k>(
        authored: &'m BTreeMap<&'k str, AuthoredDeclaration<'a>>,
        work: &mut usize,
    ) -> Result<Self, Vec<Diagnostic>> {
        let result = Self::build(authored, work);
        if result.is_err() {
            *work = 0;
        }
        result
    }
    fn build<'k>(
        authored: &'m BTreeMap<&'k str, AuthoredDeclaration<'a>>,
        work: &mut usize,
    ) -> Result<Self, Vec<Diagnostic>> {
        let count = authored.len();
        // The length walk and materialization pass, every adjacent strict-order
        // comparison and fixed reservations are charged before allocation.
        debit(work, count.checked_mul(2).ok_or_else(fail)?)?;
        let mut units = 0usize;
        let mut previous: Option<&str> = None;
        for id in authored.keys() {
            if let Some(previous) = previous {
                units = previous
                    .len()
                    .min(id.len())
                    .checked_add(1)
                    .and_then(|n| units.checked_add(n))
                    .ok_or_else(fail)?;
            }
            previous = Some(id);
        }
        debit(work, units)?;
        let bytes = count
            .checked_mul(std::mem::size_of::<Entry<'_, '_>>())
            .ok_or_else(capacity)?;
        reserve(std::mem::size_of::<Self>())?;
        reserve(bytes)?;
        let mut entries = Vec::new();
        entries.try_reserve_exact(count).map_err(|_| capacity())?;
        if entries.capacity() != count {
            return Err(capacity());
        }
        // Map order is taken as observed, never sorted here; any order that is
        // not strictly ascending refuses instead of being repaired.
        entries.extend(authored.iter().map(|(id, origin)| (*id, origin)));
        for pair in entries.windows(2) {
            if pair[0].0.cmp(pair[1].0) != Ordering::Less {
                return Err(fail());
            }
        }
        Ok(Self { entries })
    }
    /// Returns the map's own borrowed declaration; a missing identity refuses.
    pub(in super::super) fn find(
        &self,
        id: &str,
        work: &mut usize,
    ) -> Result<&'m AuthoredDeclaration<'a>, Vec<Diagnostic>> {
        let (mut low, mut high) = (0, self.entries.len());
        while low < high {
            debit(work, 1)?;
            let middle = low + (high - low) / 2;
            let (key, origin) = self.entries[middle];
            match compare(key, id, work)? {
                Ordering::Less => low = middle + 1,
                Ordering::Greater => high = middle,
                Ordering::Equal => return Ok(origin),
            }
        }
        Err(fail())
    }
}
