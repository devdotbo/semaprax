//! Bounded inline scratch, with the complete ordinary Vec spill path.
//! This bound selects storage only; no type, depth or argument is rejected.
pub(super) struct Stack<T, const N: usize> {
    inline: [Option<T>; N],
    len: usize,
    spill: Vec<T>,
}
impl<T, const N: usize> Stack<T, N> {
    pub(super) fn new() -> Self {
        Self {
            inline: std::array::from_fn(|_| None),
            len: 0,
            spill: Vec::new(),
        }
    }
    pub(super) fn push(&mut self, value: T) {
        if self.spill.is_empty() && self.len < N {
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
#[cfg(test)]
mod tests;
