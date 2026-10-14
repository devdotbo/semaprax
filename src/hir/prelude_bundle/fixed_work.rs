//! Draft fixed-schema producer authority, pending ROOT contract approval.
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
pub(crate) struct FixedSchemaPayer<'a, E> {
    reserve: &'a mut dyn FnMut(usize) -> Result<(), E>,
    limit: usize,
    remaining: usize,
    first: Option<WorkRefusal>,
}
impl<'a, E> FixedSchemaPayer<'a, E> {
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
            first: self.first,
        }
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
impl<E> Payer for FixedSchemaPayer<'_, E> {
    type Error = FixedFailure<E>;
    fn bytes(&mut self, requested: usize) -> Result<(), Self::Error> {
        if let Some(first) = self.first {
            return Err(FixedFailure::Work(first));
        }
        (self.reserve)(requested).map_err(FixedFailure::Bytes)
    }
    fn work(&mut self, requested: usize) -> Result<(), Self::Error> {
        if let Some(first) = self.first {
            return Err(FixedFailure::Work(first));
        }
        // A prior byte-owner failure must win before selecting a work refusal.
        // The original owner enforces its own sticky zero-request behavior.
        (self.reserve)(0).map_err(FixedFailure::Bytes)?;
        if requested > self.remaining {
            let first = WorkRefusal {
                requested,
                remaining: self.remaining,
            };
            self.first = Some(first);
            return Err(FixedFailure::Work(first));
        }
        self.remaining -= requested;
        Ok(())
    }
}
