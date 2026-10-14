//! Fixed-schema producer authority; root integration/qualification is pending.
//! Borrows the existing byte owner and cannot lend credit to replay or callers.
use super::{work_model, Payer};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkRefusal {
    pub(crate) requested: usize,
    pub(crate) remaining: usize,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct WorkReceipt {
    pub(crate) limit: usize,
    pub(crate) used: usize,
    pub(crate) remaining: usize,
    pub(crate) first: Option<WorkRefusal>,
}
#[derive(Debug, Eq, PartialEq)]
pub(crate) enum FixedFailure<E> {
    Bytes(E),
    Work(WorkRefusal),
}

/// One borrowed same-byte-owner account for one fixed base producer. It owns no
/// heap and no authored/replay work allowance. Keep it through construction and
/// refusal; creating a new invocation is not a retry on this failed account.
pub(crate) struct FixedSchemaPayer<'a, E: Copy> {
    reserve: &'a mut dyn FnMut(usize) -> Result<(), E>,
    limit: usize,
    remaining: usize,
    first: Option<FixedFailure<E>>,
}
impl<'a, E: Copy> FixedSchemaPayer<'a, E> {
    pub(crate) fn acquire(reserve: &'a mut dyn FnMut(usize) -> Result<(), E>) -> Result<Self, E> {
        let carriers = std::mem::size_of::<Self>()
            .checked_add(std::mem::size_of::<Result<Self, E>>())
            .and_then(|n| n.checked_add(std::mem::size_of::<WorkReceipt>()))
            .expect("fixed producer counter has addressable receiving layouts");
        reserve(carriers)?;
        Ok(Self {
            reserve,
            limit: work_model::REQUIRED,
            remaining: work_model::REQUIRED,
            first: None,
        })
    }
    pub(crate) fn receipt(&self) -> WorkReceipt {
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
    pub(crate) fn first_failure(&self) -> Option<FixedFailure<E>> {
        self.first.as_ref().map(|first| match first {
            FixedFailure::Bytes(error) => FixedFailure::Bytes(*error),
            FixedFailure::Work(receipt) => FixedFailure::Work(*receipt),
        })
    }
    #[cfg(test)]
    pub(super) fn restrict_for_test(&mut self, limit: usize) {
        assert!(limit <= work_model::REQUIRED);
        assert_eq!(self.limit, self.remaining);
        assert!(self.first.is_none());
        self.limit = limit;
        self.remaining = limit;
    }
}
impl<E: Copy> Payer for FixedSchemaPayer<'_, E> {
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
