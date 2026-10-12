//! Exact owned continuation labels: count and reserve before rendering.
use super::*;
use std::fmt::{Arguments, Write};
struct Count(usize);
impl Write for Count {
    fn write_str(&mut self, value: &str) -> std::fmt::Result {
        self.0 = self.0.checked_add(value.len()).ok_or(std::fmt::Error)?;
        Ok(())
    }
}
pub(in crate::hir::validation) fn format(arguments: Arguments<'_>) -> Result<String, Diagnostic> {
    let mut count = Count(0);
    count
        .write_fmt(arguments)
        .map_err(|_| charge::failure(usize::MAX, "scope continuation label layout overflowed"))?;
    charge::reserve(Some(count.0))?;
    let mut result = String::with_capacity(count.0);
    if result.capacity() > count.0 {
        charge::reserve(Some(result.capacity() - count.0))?;
    }
    result
        .write_fmt(arguments)
        .expect("exact internal continuation label");
    Ok(result)
}
#[cfg(test)]
pub(in crate::hir::validation) fn owned(value: &str) -> Result<String, Diagnostic> {
    format(format_args!("{value}"))
}

// Called only on the missing-value branch of lazy diagnostic closures. A
// physical refusal remains the selected diagnostic instead of being mapped
// back into a later semantic error.
pub(in crate::hir::validation) fn diagnostic(arguments: Arguments<'_>) -> Diagnostic {
    match format(arguments) {
        Ok(message) => hir_error(message),
        Err(error) => error,
    }
}
