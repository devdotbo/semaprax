//! Test-only, allocation-free debit attribution for one exact resolved-core
//! ledger. Counters describe reservations, never physical allocation or refunds.
use super::{active, Budget, ReservationStage};
use std::cell::Cell;
use std::rc::Rc;

const COUNT: usize = ReservationStage::RetainedLoanSidecars as usize + 2;
#[derive(Clone, Copy, Debug)]
pub(crate) struct Trace {
    owner: usize,
    pub(crate) initial: usize,
    pub(crate) consumed: usize,
    pub(crate) prebound: usize,
    pub(crate) layouts: usize,
    pub(crate) mode: u8,
    pub(crate) debits: [usize; COUNT],
}
impl Trace {
    const EMPTY: Self = Self {
        owner: 0,
        initial: 0,
        consumed: 0,
        prebound: 0,
        layouts: 0,
        mode: 0,
        debits: [0; COUNT],
    };
    pub(crate) fn sum(self) -> usize {
        self.debits.into_iter().sum()
    }
}
thread_local! {
    static CURRENT: Cell<Trace> = const { Cell::new(Trace::EMPTY) };
    static COMPLETED: Cell<Option<Trace>> = const { Cell::new(None) };
}
// No Budget field or allocation: address equality only selects this diagnostic
// observation scope and grants no source, identity, or budget authority.
pub(super) fn debit(budget: &Budget, bytes: usize) {
    CURRENT.with(|current| {
        let mut trace = current.get();
        if trace.owner == budget as *const Budget as usize {
            let ordinal = budget.stage.get().map_or(0, |stage| stage as usize + 1);
            trace.debits[ordinal] = trace.debits[ordinal]
                .checked_add(bytes)
                .expect("stage debit overflow cannot exceed the finite budget");
            current.set(trace);
        }
    });
}
pub(crate) struct Guard {
    previous: Trace,
}
pub(crate) fn start(prebound: usize, layouts: usize, mode: u8) -> Guard {
    let budget = active().expect("resolved core has its existing budget");
    let trace = Trace {
        owner: Rc::as_ptr(&budget) as usize,
        initial: budget.remaining.get(),
        prebound,
        layouts,
        mode,
        ..Trace::EMPTY
    };
    let previous = CURRENT.with(|current| current.replace(trace));
    Guard { previous }
}
impl Drop for Guard {
    fn drop(&mut self) {
        let mut trace = CURRENT.with(|current| current.replace(self.previous));
        let budget = active().expect("core trace settles before its ledger");
        assert_eq!(trace.owner, Rc::as_ptr(&budget) as usize);
        trace.consumed = trace.initial - budget.remaining.get();
        assert_eq!(
            trace.sum(),
            trace.consumed,
            "every committed debit exactly once"
        );
        COMPLETED.with(|completed| {
            if completed
                .get()
                .is_none_or(|old| trace.consumed > old.consumed)
            {
                completed.set(Some(trace));
            }
        });
    }
}
pub(crate) fn take_completed() -> Option<Trace> {
    COMPLETED.with(|completed| completed.take())
}

#[cfg(test)]
mod tests;
