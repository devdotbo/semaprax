//! New explicit finite extension work owner; base-two work is unchanged.
//! Borrows the existing byte owner and cannot lend credit to replay or callers.
use super::fixed_work::{FixedFailure, WorkReceipt, WorkRefusal};
use super::Payer;

// Exact actual work sites in vector_extension::acquire: entry, kind and
// parameter-shape branches, parameter initializer, identity copy, and two
// UTF-8/name byte passes for each of the type and parameter names.
#[cfg(test)]
pub(super) const VECTOR_WORK: usize =
    4 + crate::prelude::VEC_ID.len() + 2 * "Vec".len() + 2 * "T".len();

/// One borrowed same-byte-owner account for one selected extension slot. It owns no
/// heap and no authored/replay work allowance. Keep it through construction and
/// refusal; creating a new invocation is not a retry on this failed account.
pub(super) struct ExtensionPayer<'a, E: Copy> {
    reserve: &'a mut dyn FnMut(usize) -> Result<(), E>,
    limit: usize,
    remaining: usize,
    first: Option<FixedFailure<E>>,
}
impl<'a, E: Copy> ExtensionPayer<'a, E> {
    #[cfg(test)]
    pub(super) fn acquire(reserve: &'a mut dyn FnMut(usize) -> Result<(), E>) -> Result<Self, E> {
        Self::acquire_for(0, reserve)
    }
    pub(super) fn acquire_for(
        slot: usize,
        reserve: &'a mut dyn FnMut(usize) -> Result<(), E>,
    ) -> Result<Self, E> {
        let limit = super::extension_model::WORK[slot];
        let carriers = std::mem::size_of::<Self>()
            .checked_add(std::mem::size_of::<Result<Self, E>>())
            .and_then(|n| n.checked_add(std::mem::size_of::<WorkReceipt>()))
            .expect("fixed producer counter has addressable receiving layouts");
        reserve(carriers)?;
        Ok(Self {
            reserve,
            limit,
            remaining: limit,
            first: None,
        })
    }
    pub(super) fn receipt(&self) -> WorkReceipt {
        WorkReceipt {
            limit: self.limit,
            used: self.limit - self.remaining,
            remaining: self.remaining,
            first: match self.first.as_ref() {
                Some(FixedFailure::Work(first)) => Some(*first),
                _ => None,
            },
        }
    }
    /// Copy the retained inline payload; no Clone requirement, formatting or
    /// callback requery can change an already selected byte/work error.
    pub(super) fn first_failure(&self) -> Option<FixedFailure<E>> {
        self.first.as_ref().map(|first| match first {
            FixedFailure::Bytes(error) => FixedFailure::Bytes(*error),
            FixedFailure::Work(receipt) => FixedFailure::Work(*receipt),
        })
    }
    #[cfg(test)]
    pub(super) fn restrict_for_test(&mut self, limit: usize) {
        assert!(limit <= self.limit);
        assert_eq!(self.limit, self.remaining);
        assert!(self.first.is_none());
        self.limit = limit;
        self.remaining = limit;
    }
}
impl<E: Copy> Payer for ExtensionPayer<'_, E> {
    type Error = FixedFailure<E>;
    fn bytes(&mut self, requested: usize) -> Result<(), Self::Error> {
        if let Some(first) = self.first_failure() {
            return Err(first);
        }
        match (self.reserve)(requested) {
            Ok(()) => Ok(()),
            Err(error) => {
                self.first = Some(FixedFailure::Bytes(error));
                Err(FixedFailure::Bytes(error))
            }
        }
    }
    fn work(&mut self, requested: usize) -> Result<(), Self::Error> {
        if let Some(first) = self.first_failure() {
            return Err(first);
        }
        // Authenticate the original byte owner before selecting work failure,
        // retaining its exact inline E even if its later callback taxonomy changes.
        self.bytes(0)?;
        if requested > self.remaining {
            let first = WorkRefusal {
                requested,
                remaining: self.remaining,
            };
            self.first = Some(FixedFailure::Work(first));
            return Err(FixedFailure::Work(first));
        }
        self.remaining -= requested;
        Ok(())
    }
}
