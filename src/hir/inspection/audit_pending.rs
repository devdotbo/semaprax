//! Borrowed LIFO frames for identity audits. This is ordinary scratch storage,
//! not authenticated funding: the unbounded Vec spill remains an open payer.
//! Push and extend retain the original walker order, including when new children
//! arrive while an older spill is still pending. No depth or work limit is added.

pub(super) const INLINE: usize = 32;

pub(super) struct Pending<'a, T> {
    inline: [Option<&'a T>; INLINE],
    len: usize,
    spill: Vec<&'a T>,
}

impl<'a, T> Pending<'a, T> {
    pub(super) fn new(root: &'a T) -> Self {
        let mut pending = Self {
            inline: [None; INLINE],
            len: 0,
            spill: Vec::new(),
        };
        pending.push(root);
        pending
    }

    pub(super) fn push(&mut self, value: &'a T) {
        if self.len < INLINE && self.spill.is_empty() {
            self.inline[self.len] = Some(value);
            self.len += 1;
        } else {
            self.spill.push(value);
        }
    }

    pub(super) fn extend(&mut self, values: impl IntoIterator<Item = &'a T>) {
        for value in values {
            self.push(value);
        }
    }

    pub(super) fn pop(&mut self) -> Option<&'a T> {
        if let Some(value) = self.spill.pop() {
            return Some(value);
        }
        if self.len == 0 {
            return None;
        }
        self.len -= 1;
        self.inline[self.len].take()
    }
}
