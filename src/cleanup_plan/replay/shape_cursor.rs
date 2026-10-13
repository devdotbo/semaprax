//! Borrowed shape events: inline common path and prepaid reusable spill.
//! Storage changes only; the complete ordered shape receiver remains in replay.
use super::*;
const INLINE: usize = 32;
#[derive(Clone, Copy)]
pub(super) enum Item<'a> {
    Shape(&'a FieldLivenessShape),
    Push(&'a DeclarationId),
    Pop,
}
pub(super) struct Pending<'a> {
    inline: [Option<Item<'a>>; INLINE],
    len: usize,
    spill: Vec<Item<'a>>,
}
impl<'a> Pending<'a> {
    pub(super) fn new() -> Self {
        Self {
            inline: [None; INLINE],
            len: 0,
            spill: Vec::new(),
        }
    }
    pub(super) fn reserve(
        &mut self,
        function: &ResolvedFunction,
        budget: &mut ReplayBudget,
        additional: usize,
        phase: &str,
    ) -> Result<(), Diagnostic> {
        // An inline event allocates no heap backing, but cannot bypass a
        // prior sticky refusal. The spill uses the ordinary actual payer.
        budget.reserve_bytes(function, 0, phase)?;
        let available = if self.spill.is_empty() {
            INLINE - self.len
        } else {
            0
        };
        let spill_additional = additional.saturating_sub(available);
        budget.reserve_vec(function, &mut self.spill, spill_additional, phase)?;
        // The unobserved compatibility receiver still needs fallible growth;
        // the paid route already acquired this exact backing before this call.
        self.spill.try_reserve(spill_additional).map_err(|_| {
            replay_error(
                function,
                "cleanup shape replay capacity exceeds address space",
            )
        })?;
        Ok(())
    }
    pub(super) fn push(&mut self, item: Item<'a>) {
        if self.spill.is_empty() && self.len < INLINE {
            self.inline[self.len] = Some(item);
            self.len += 1;
        } else {
            // Every caller reserves its complete authored event group first.
            assert!(self.spill.len() < self.spill.capacity());
            self.spill.push(item);
        }
    }
    pub(super) fn pop(&mut self) -> Option<Item<'a>> {
        if let Some(item) = self.spill.pop() {
            return Some(item);
        }
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        self.inline[self.len].take()
    }
}
