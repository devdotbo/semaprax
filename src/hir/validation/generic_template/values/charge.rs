//! Literal current control/initializer and retained Rc node backing.
use super::*;
use std::alloc::Layout;
#[repr(C)]
#[allow(dead_code, reason = "exact Rc carrier layout mirror")]
struct Carrier<T> {
    strong: usize,
    weak: usize,
    value: T,
}
fn reserve(bytes: Option<usize>) -> Result<(), Diagnostic> {
    let Some(bytes) = bytes else {
        crate::bounded_output::refuse_active_required(usize::MAX);
        return Err(hir_error("generic value checkpoint layout overflowed"));
    };
    if crate::bounded_output::reserve_active_required(bytes) {
        Ok(())
    } else {
        Err(hir_error(
            "generic value checkpoint exceeds the active validation budget",
        ))
    }
}
// Allocation-free unique replacements must still honor a selected refusal.
pub(super) fn active() -> Result<(), Diagnostic> {
    reserve(Some(0))
}
pub(super) fn control() -> Result<(), Diagnostic> {
    reserve(Some(std::mem::size_of::<Values<'_>>()))
}
pub(super) fn node() -> Result<(), Diagnostic> {
    // The key's Arc clone shares existing immutable identity bytes. Its handle
    // and the borrowed type pointer are contained in Node, not unpaid payloads.
    reserve(
        Layout::new::<Carrier<tree::Node<'_>>>()
            .size()
            .checked_add(std::mem::size_of::<tree::Node<'_>>()),
    )
}
