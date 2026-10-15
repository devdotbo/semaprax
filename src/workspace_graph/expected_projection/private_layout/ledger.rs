//! Shared invocation work and sticky refusal; the physical carrier is precharged.
use super::reserve_builder_structure;
use crate::diagnostic::Diagnostic;
use std::{cell::Cell, rc::Rc};
pub(super) struct Ledger {
    pub(super) remaining: Cell<usize>,
    pub(super) failed: Cell<bool>,
}
impl Ledger {
    pub(super) fn new(remaining: usize) -> Result<Rc<Self>, Vec<Diagnostic>> {
        // Pinned Rc inner layout: strong/weak usize counts followed by this
        // usize-aligned, fixed-size payload. No dynamically owned fields.
        #[repr(C)]
        struct Carrier {
            strong: usize,
            weak: usize,
            value: Ledger,
        }
        reserve_builder_structure(std::mem::size_of::<Carrier>())?;
        Ok(Rc::new(Self {
            remaining: Cell::new(remaining),
            failed: Cell::new(false),
        }))
    }
}
