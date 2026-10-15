//! Explicit invocation acquisition. No ambient ledger or diagnostic construction.
use super::{Failure, Payer};
use crate::hir::DeclarationId;
use std::alloc::Layout;

pub(super) fn vector<T, P: Payer>(
    count: usize,
    payer: &mut P,
) -> Result<Vec<T>, Failure<P::Error>> {
    let layout = Layout::array::<T>(count).map_err(|_| Failure::Layout)?;
    payer.bytes(layout.size()).map_err(Failure::Payer)?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| Failure::Allocation)?;
    // No downstream growth or allocator capacity repair may become unpaid.
    if values.capacity() != count && std::mem::size_of::<T>() != 0 {
        return Err(Failure::Capacity);
    }
    Ok(values)
}

pub(super) fn string<P: Payer>(text: &str, payer: &mut P) -> Result<String, Failure<P::Error>> {
    payer.work(text.len()).map_err(Failure::Payer)?;
    let mut bytes = vector::<u8, _>(text.len(), payer)?;
    bytes.extend_from_slice(text.as_bytes());
    payer.work(text.len()).map_err(Failure::Payer)?;
    String::from_utf8(bytes).map_err(|_| Failure::Schema)
}

pub(super) fn identity<P: Payer>(
    text: &str,
    payer: &mut P,
) -> Result<DeclarationId, Failure<P::Error>> {
    payer.work(text.len()).map_err(Failure::Payer)?;
    DeclarationId::paid_borrowed(text, |bytes| payer.bytes(bytes)).map_err(Failure::Payer)
}

pub(super) fn shared_bytes<T>() -> usize {
    #[repr(C)]
    struct Carrier<T> {
        strong: usize,
        weak: usize,
        value: T,
    }
    Layout::new::<Carrier<T>>().size()
}
