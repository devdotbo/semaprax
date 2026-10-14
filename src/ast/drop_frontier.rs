//! Infallible destruction scratch: inline slots are a representation choice,
//! never an admission/depth limit. Wider frontiers spill to ordinary Vec backing.
//! Spill remains an explicit unpriced compatibility boundary; Drop never consults
//! a payer, refuses a release, skips a child, or falls back to recursive teardown.
const INLINE: usize = 16;

pub(super) struct Stack<T> {
    inline: [Option<T>; INLINE],
    len: usize,
    spill: Vec<T>,
}
impl<T> Stack<T> {
    pub(super) fn new() -> Self {
        Self {
            inline: std::array::from_fn(|_| None),
            len: 0,
            spill: Vec::new(),
        }
    }
    pub(super) fn push(&mut self, value: T) {
        if self.spill.is_empty() && self.len < INLINE {
            self.inline[self.len] = Some(value);
            self.len += 1;
        } else {
            self.spill.push(value);
        }
    }
    pub(super) fn pop(&mut self) -> Option<T> {
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
impl<T> Extend<T> for Stack<T> {
    fn extend<I: IntoIterator<Item = T>>(&mut self, values: I) {
        for value in values {
            self.push(value);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn inline_spill_reentry_and_extend_keep_complete_vec_lifo_and_every_value() {
        let mut actual = Stack::new();
        let mut original = Vec::new();
        for value in 0..INLINE + 4 {
            actual.push(value);
            original.push(value);
        }
        for _ in 0..6 {
            assert_eq!(actual.pop(), original.pop());
        }
        let retained = actual.spill.capacity();
        for value in 100..106 {
            actual.push(value);
            original.push(value);
        }
        assert_eq!(actual.spill.capacity(), retained);
        actual.extend(200..257);
        original.extend(200..257);
        while let Some(expected) = original.pop() {
            assert_eq!(actual.pop(), Some(expected));
        }
        assert_eq!(actual.pop(), None);
    }
    #[test]
    fn inline_frontiers_acquire_no_heap_for_empty_and_small_receivers() {
        let (_, metrics) = crate::test_allocation_observer::measure(|| {
            let mut values = Stack::new();
            values.extend(0..INLINE);
            for expected in (0..INLINE).rev() {
                assert_eq!(values.pop(), Some(expected));
            }
            assert_eq!(values.pop(), None);
            drop(values);
        });
        assert!(!metrics.invalid);
        assert_eq!((metrics.allocated, metrics.live), (0, 0));
    }
}
