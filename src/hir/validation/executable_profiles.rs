//! Validator-local memo of pure executable owned-text profile observations.
//!
//! Arbitrary types retain the exact zero-argument record memo, keyed by the
//! retained index's declaration identity. The additive held-parameter route
//! binds generic observations to a genuine immutable program/ordinal lookup;
//! arbitrary, generated, unknown and mismatched inputs keep the full paid route.
//! Every request and every hit authenticates the ledger with a zero-byte
//! reservation, so a hit can never outlive a refusal.
use super::*;
use crate::hir::owned_text_record::paid;
use crate::hir::DeclarationIndex;
use std::cell::RefCell;
use std::rc::Rc;

#[cfg(test)]
mod tests;

#[derive(Clone, Copy)]
enum Observation {
    Contains,
    Runtime,
}

// Keys borrow the retained declaration identity, never the caller's type.
struct Entry<'a> {
    id: &'a DeclarationId,
    contains: Option<bool>,
    runtime: Option<bool>,
}

impl Entry<'_> {
    fn get(&self, which: Observation) -> Option<bool> {
        match which {
            Observation::Contains => self.contains,
            Observation::Runtime => self.runtime,
        }
    }
    fn set(&mut self, which: Observation, value: bool) {
        match which {
            Observation::Contains => self.contains = Some(value),
            Observation::Runtime => self.runtime = Some(value),
        }
    }
}

// Sorted by identity; each growth is paid at its full new capacity.
struct State<'a> {
    entries: Vec<Entry<'a>>,
    bound: Option<held_profiles::Bound<'a>>,
}

#[derive(Clone)]
pub(super) struct Lookup<'a> {
    index: &'a DeclarationIndex,
    state: Rc<RefCell<State<'a>>>,
}

impl<'a> Lookup<'a> {
    pub(super) fn new(index: &'a DeclarationIndex) -> Result<Self, Diagnostic> {
        paid::fund(Some(0))?;
        paid::fund(Some(Self::control_bytes()))?;
        Ok(Self {
            index,
            state: Rc::new(RefCell::new(State {
                entries: Vec::new(),
                bound: None,
            })),
        })
    }

    // The strong and weak counters precede the RefCell in the Rc allocation.
    fn control_bytes() -> usize {
        std::alloc::Layout::new::<[usize; 2]>()
            .extend(std::alloc::Layout::new::<RefCell<State<'a>>>())
            .expect("classifier control block has a fixed representable layout")
            .0
            .pad_to_align()
            .size()
    }

    pub(super) fn is_current(&self, index: &DeclarationIndex) -> bool {
        std::ptr::eq(self.index, index)
    }

    /// The retained identity of an exact known zero-argument record, if any.
    fn exact_key(&self, ty: &ResolvedType) -> Option<&'a DeclarationId> {
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty
        else {
            return None;
        };
        if !arguments.is_empty() {
            return None;
        }
        let index: &'a DeclarationIndex = self.index;
        let item = index.declaration(declaration)?;
        if item.kind != DeclarationKind::Record || item.id != *declaration {
            return None;
        }
        index
            .type_parameters(declaration)
            .is_some_and(|parameters| parameters.is_empty())
            .then_some(&item.id)
    }

    fn compute(&self, ty: &ResolvedType, which: Observation) -> Result<bool, Diagnostic> {
        match which {
            Observation::Contains => paid::contains_string(ty, self.index),
            Observation::Runtime => paid::runtime_admitted(ty, self.index),
        }
    }

    // The borrow ends before any classifier runs.
    fn recall(&self, key: &DeclarationId, which: Observation) -> Option<bool> {
        let state = self.state.borrow();
        let at = state
            .entries
            .binary_search_by(|entry| entry.id.cmp(key))
            .ok()?;
        state.entries[at].get(which)
    }

    // Only successful pure results arrive here. Another validator clone may have
    // inserted the same identity meanwhile; results are equal, so it is reused.
    fn remember(
        &self,
        key: &'a DeclarationId,
        which: Observation,
        value: bool,
    ) -> Result<(), Diagnostic> {
        let mut state = self.state.borrow_mut();
        match state.entries.binary_search_by(|entry| entry.id.cmp(key)) {
            Ok(at) => state.entries[at].set(which, value),
            Err(at) => {
                paid::reserve(&mut state.entries, 1)?;
                let mut entry = Entry {
                    id: key,
                    contains: None,
                    runtime: None,
                };
                entry.set(which, value);
                state.entries.insert(at, entry);
            }
        }
        Ok(())
    }

    fn observe(&self, ty: &ResolvedType, which: Observation) -> Result<bool, Diagnostic> {
        paid::fund(Some(0))?;
        let Some(key) = self.exact_key(ty) else {
            return self.compute(ty, which);
        };
        if let Some(known) = self.recall(key, which) {
            return Ok(known);
        }
        let value = self.compute(ty, which)?;
        self.remember(key, which, value)?;
        Ok(value)
    }

    /// Same verdict and diagnostic as the owned `validate_use`. The runtime
    /// observation is evaluated only when the type carries text.
    pub(super) fn validate_use(&self, ty: &ResolvedType) -> Result<(), Diagnostic> {
        paid::fund(Some(0))?;
        if self.exact_key(ty).is_none() {
            return paid::validate_use(ty, self.index);
        }
        if self.observe(ty, Observation::Contains)? && !self.observe(ty, Observation::Runtime)? {
            return Err(paid::outside_profile());
        }
        Ok(())
    }

    pub(super) fn runtime_admitted(&self, ty: &ResolvedType) -> Result<bool, Diagnostic> {
        self.observe(ty, Observation::Runtime)
    }

    #[cfg(test)]
    pub(super) fn owned_capacity(&self) -> usize {
        Self::control_bytes()
            + self.state.borrow().entries.capacity() * std::mem::size_of::<Entry<'a>>()
            + self
                .state
                .borrow()
                .bound
                .as_ref()
                .map_or(0, |bound| bound.owned_capacity())
    }
}

#[cfg(test)]
mod before_812;
mod held;

#[cfg(test)]
pub(in crate::hir::validation) fn saved_observations_812(
    index: &DeclarationIndex,
    ty: &ResolvedType,
) -> (Result<bool, Diagnostic>, Result<(), Diagnostic>) {
    let lookup = before_812::Lookup::new(index).unwrap();
    assert!(lookup.is_current(index));
    assert!(lookup.owned_capacity() > 0);
    (lookup.runtime_admitted(ty), lookup.validate_use(ty))
}
