//! Demand-backed cursor stack for the consuming traversal.
//! Eight frames live inline; deeper frames spill into an exactly-charged Vec.
use super::super::{Admission, Refusal};
use crate::hir::lexical_exit::ChildCursor;
use std::mem::size_of;

/// Original cap: MAX_DEPTH + 1 simultaneous frames.
pub(super) const LIMIT: usize = crate::cache_codec::MAX_DEPTH + 1;
pub(super) const INLINE: usize = 8;
const OVERFLOW: usize = LIMIT - INLINE;

pub(super) struct Stack<'a> {
    inline: [Option<ChildCursor<'a>>; INLINE],
    inline_len: usize,
    overflow: Vec<ChildCursor<'a>>,
}
impl<'a> Stack<'a> {
    /// Pays the real header plus inline backing once, then the original
    /// LIMIT initialization units. Backing is charged before work.
    pub(super) fn new(admission: &mut Admission<'_>) -> Result<Self, Refusal> {
        admission.reserve(size_of::<Self>())?;
        admission.units(LIMIT)?;
        Ok(Self {
            inline: [None; INLINE],
            inline_len: 0,
            overflow: Vec::new(),
        })
    }
    pub(super) fn is_empty(&self) -> bool {
        self.inline_len == 0
    }
    pub(super) fn depth(&self) -> usize {
        self.inline_len + self.overflow.len()
    }
    pub(super) fn top(&mut self) -> &mut ChildCursor<'a> {
        match self.overflow.last_mut() {
            Some(frame) => frame,
            None => self.inline[self.inline_len - 1]
                .as_mut()
                .expect("occupied consuming directory frame"),
        }
    }
    pub(super) fn push(
        &mut self,
        cursor: ChildCursor<'a>,
        admission: &mut Admission<'_>,
    ) -> Result<(), Refusal> {
        if self.depth() == LIMIT {
            return admission.refuse(Refusal::Capacity);
        }
        if self.inline_len < INLINE {
            self.inline[self.inline_len] = Some(cursor);
            self.inline_len += 1;
            return Ok(());
        }
        if self.overflow.len() == self.overflow.capacity() {
            self.grow(admission)?;
        }
        self.overflow.push(cursor);
        Ok(())
    }
    pub(super) fn pop(&mut self) {
        if self.overflow.pop().is_none() {
            self.inline_len -= 1;
            self.inline[self.inline_len] = None;
        }
    }
    /// Each full replacement header and backing is charged before allocation; the old
    /// backing stays installed until the exact replacement exists. Moving at
    /// most OVERFLOW frames is covered by the LIMIT initialization units.
    fn grow(&mut self, admission: &mut Admission<'_>) -> Result<(), Refusal> {
        let doubled = admission.arithmetic(self.overflow.capacity().checked_mul(2))?;
        let count = doubled.max(INLINE).min(OVERFLOW);
        let cell = size_of::<ChildCursor<'a>>();
        let bytes = admission.arithmetic(
            count
                .checked_mul(cell)
                .and_then(|bytes| bytes.checked_add(size_of::<Vec<ChildCursor<'a>>>())),
        )?;
        admission.reserve(bytes)?;
        let mut replacement: Vec<ChildCursor<'a>> = Vec::new();
        if replacement.try_reserve_exact(count).is_err() {
            return admission.refuse(Refusal::Allocation);
        }
        if replacement.capacity() != count {
            if replacement.capacity() > count {
                let excess =
                    admission.arithmetic((replacement.capacity() - count).checked_mul(cell))?;
                admission.reserve(excess)?;
            }
            return admission.refuse(Refusal::Capacity);
        }
        replacement.append(&mut self.overflow);
        self.overflow = replacement;
        Ok(())
    }
}
