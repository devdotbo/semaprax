//! Actual selected Core-Wasm host representations.
//!
//! These are the exact typed-array byte lengths `host.mjs` debits from one
//! caller-held, preallocated, never-reset invocation account `ArrayBuffer`
//! before any payload allocation: the registry entries it precharges (whose
//! storage is the realm namespace's preallocated registry buffer, so this
//! debit is a conservative second reservation), then the carved row backing
//! and lease/view tables. JavaScript Map, closure and object bookkeeping is
//! not counted and is not claimed as a physical reservation.

/// Simultaneous borrowed payload views; matches the reference domain.
pub(super) const MAX_VIEWS: u64 = crate::construction_fusion::domain::MAX_VIEWS as u64;
/// Per lease slot: nonce u64, generation u64, live u8, published u8, borrows u32.
pub(super) const SLOT_BYTES: u64 = 8 + 8 + 1 + 1 + 4;
/// Per view slot: nonce u64, owner slot u32, generation u64, live u8.
pub(super) const VIEW_BYTES: u64 = 8 + 4 + 8 + 1;
/// Host row carrier: scalar i64, String handle u64, Bytes owner handle u64.
/// This is the selected layout; the legacy owned-leaf row is not reinterpreted.
pub(super) const ROW_BYTES: u64 = 8 + 8 + 8;
/// Per namespace registry entry: token u64; host, instance, invocation,
/// domain, target and free-list u32; kind u8.
pub(super) const REGISTRY_ENTRY_BYTES: u64 = 8 + 4 * 6 + 1;
/// Ordinary clones a domain can mint before the 32-event limit refuses one.
const MAX_CLONES: u64 = crate::byte_data_capacity::MAX_BYTES_COPY_SITES as u64 - 1;
/// Owned leaves per selected row (String and Bytes), each charged at the
/// ordinary owned-leaf Vec per-element rate.
const OWNED_LEAVES: u64 = 2;

/// Complete lease and view metadata carved at the first Bytes operand.
pub(super) fn metadata_bytes(rows: u32) -> u64 {
    u64::from(rows) * SLOT_BYTES + MAX_VIEWS * VIEW_BYTES
}

/// Exact per-domain account debit, in order: the precharged registry entries'
/// byteLength at `start` (the original Vec boundary), the row backing carved
/// at the Vec allocation, then the lease/view tables carved at the first
/// Bytes operand. Carving order is 8-byte, then 4-byte, then 1-byte arrays,
/// with exact charged trailing alignment to eight bytes for the next carve or invocation. The account never resets or refunds, so each
/// invocation on one account consumes this again.
pub(super) fn account_bytes(rows: u32) -> u64 {
    (registry_entries(rows) * REGISTRY_ENTRY_BYTES).next_multiple_of(8)
        + u64::from(rows) * ROW_BYTES
        + metadata_bytes(rows).next_multiple_of(8)
}

/// Registry entries precharged at `start` for the domain's maximum live
/// handles: the domain and its Vec, one String and one owner per row, one
/// popped-row handle per row, every view, every possible ordinary clone, and
/// one entry of renewal overlap (a new token is minted before the old retires).
pub(super) fn registry_entries(rows: u32) -> u64 {
    let rows = u64::from(rows);
    2 + 3 * rows + MAX_VIEWS + MAX_CLONES + 1
}

/// The selected row Vec must fit the unchanged ordinary Vec capacity and
/// owned-payload envelopes for its actual leaves. The strategy's row ceiling
/// alone authorizes no carrier layout.
pub(super) fn fits_ordinary_vec(rows: u32) -> bool {
    let rows = u64::from(rows);
    rows <= crate::vec_ops::MAX_CAPACITY
        && rows
            .checked_mul(OWNED_LEAVES * crate::vec_ops::OWNED_PAYLOAD_BYTES_PER_ELEMENT)
            .is_some_and(|bytes| bytes <= crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES)
}
