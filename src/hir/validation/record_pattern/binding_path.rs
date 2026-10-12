//! Replays exactly the active DFS field indices, without a second directory.
use super::*;
use std::fmt::{self, Write};
pub(super) struct BindingPath<'a, I> {
    root: &'a dyn fmt::Display,
    indices: I,
}
impl<'a, I: Iterator<Item = usize> + Clone> BindingPath<'a, I> {
    pub(super) fn new(root: &'a dyn fmt::Display, indices: I) -> Self {
        Self { root, indices }
    }
    pub(super) fn len(&self) -> Result<usize, Diagnostic> {
        struct Count(usize);
        impl Write for Count {
            fn write_str(&mut self, text: &str) -> fmt::Result {
                self.0 = self.0.checked_add(text.len()).ok_or(fmt::Error)?;
                Ok(())
            }
        }
        let mut count = Count(0);
        write!(count, "{self}").map_err(|_| {
            scope_checkpoint::charge::failure(
                usize::MAX,
                "scope continuation label layout overflowed",
            )
        })?;
        Ok(count.0)
    }
}
impl<I: Iterator<Item = usize> + Clone> fmt::Display for BindingPath<'_, I> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.root)?;
        let mut indices = self.indices.clone().peekable();
        while let Some(index) = indices.next() {
            write!(f, ".field.{index}")?;
            f.write_str(if indices.peek().is_some() {
                ".record"
            } else {
                ".binding"
            })?;
        }
        Ok(())
    }
}
