//! Explicit empty closures carry semantic success without a map/Rc allocation.
//! Overflow remains the enclosing None; cached Empty remains a present memo key.
use super::ordered_weights::{Iter, OrderedWeights};
#[cfg(test)]
use super::{Diagnostic, Rc, ReplayBudget, ResolvedFunction};
#[cfg(test)]
use std::mem::size_of;

#[cfg(test)]
#[derive(Clone, Debug)]
pub(super) enum Closure {
    Empty,
    Shared(Rc<OrderedWeights>),
}
#[cfg(test)]
impl PartialEq for Closure {
    fn eq(&self, other: &Self) -> bool {
        self.iter().eq(other.iter())
    }
}
#[cfg(test)]
impl Eq for Closure {}
#[cfg(test)]
impl Closure {
    // Outcome adapter only exists for existing cfg(test) Result-based controls.
    // Two enum values + two outer Option receivers + added adapter Result,
    // actual payment Result and its function/budget argument receivers.
    pub(super) const ADAPTER_CONTROLS: usize = 2 * size_of::<Self>()
        + 2 * size_of::<Option<Self>>()
        + size_of::<Result<Option<Self>, Diagnostic>>()
        + size_of::<Result<(), Diagnostic>>()
        + size_of::<&ResolvedFunction>()
        + size_of::<&mut ReplayBudget>();
    pub(super) fn len(&self) -> usize {
        match self {
            Self::Empty => 0,
            Self::Shared(weights) => weights.len(),
        }
    }
    pub(super) fn iter(&self) -> Iter<'_> {
        // One original iterator result; no wrapper iterator or collected copy.
        match self {
            Self::Empty => Iter::empty(),
            Self::Shared(weights) => weights.iter(),
        }
    }
    pub(super) fn get(&self, key: &usize) -> Option<&u64> {
        match self {
            Self::Empty => None,
            Self::Shared(weights) => weights.get(key),
        }
    }
    #[cfg(test)]
    pub(super) fn same_storage(left: &Self, right: &Self) -> bool {
        match (left, right) {
            (Self::Empty, Self::Empty) => true,
            (Self::Shared(left), Self::Shared(right)) => Rc::ptr_eq(left, right),
            _ => false,
        }
    }
}

// A directly paid borrowed common view replaces the old borrowed map reference.
// It has no owning map, allocation, iterator adapter, or persistent lifetime.
pub(super) enum View<'a> {
    Empty,
    Shared(&'a OrderedWeights),
}
impl<'a> View<'a> {
    pub(super) fn get(&self, key: &usize) -> Option<&'a u64> {
        match self {
            Self::Empty => None,
            Self::Shared(weights) => weights.get(key),
        }
    }
    pub(super) fn iter(&self) -> Iter<'a> {
        match self {
            Self::Empty => Iter::empty(),
            Self::Shared(weights) => weights.iter(),
        }
    }
}
