//! Shared immutable custody publication under the SAME captured prepare owner.
use super::*;
use std::rc::Rc;
fn bytes<T>(payer: Payer<'_>) -> Result<usize, Diagnostic> {
    // Rc's two reference counters precede its aligned value. The owning
    // allocator control pins this model against the actual Rust allocation.
    std::alloc::Layout::new::<[usize; 2]>()
        .extend(std::alloc::Layout::new::<T>())
        .map(|(layout, _)| layout.pad_to_align().size())
        .map_err(|_| payer.capacity(usize::MAX, 0))
}
pub(super) fn seal<'a>(
    storage: Storage<'a>,
    payer: Payer<'_>,
) -> Result<Rc<Storage<'a>>, Diagnostic> {
    payer.reserve(bytes::<Storage<'a>>(payer)?)?;
    // Finite invocation refusal returns before Rc acquires backing. Process
    // allocator exhaustion retains ordinary Rc/BTree allocation OOM behavior.
    Ok(Rc::new(storage))
}
#[cfg(test)]
mod tests;
