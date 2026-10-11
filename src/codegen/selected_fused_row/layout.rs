//! Native64 mirror of the selected runtime's carriers, owned by this receiver.
//!
//! The retired single-object precursor context mirror in
//! `construction_fusion::native_provenance` is not used by this route. Every
//! size here is emitted as a static assertion beside the runtime's own, so a
//! drift between this mirror and the C runtime fails the selected unit.
#![allow(
    dead_code,
    reason = "repr(C) mirror fields exist only for their target layout"
)]
use std::mem::size_of;

#[repr(C)]
struct Issuer {
    module: *const (),
    next: u64,
    active: *const (),
    sealed: u32,
}
#[repr(C)]
struct Ledger {
    limit: usize,
    debited: usize,
    held_rows: usize,
    sealed: u32,
    failed: u32,
}
#[repr(C)]
struct Payload {
    events: usize,
    cumulative: usize,
    held_events: usize,
    held_bytes: usize,
    sealed: u32,
    failed: u32,
}
#[repr(C)]
struct Bytes {
    issuer: *const (),
    slot: u32,
    generation: u32,
    nonce: u64,
}
#[repr(C)]
struct Stored {
    slot: u32,
    generation: u32,
    nonce: u64,
}
#[repr(C)]
struct View {
    issuer: *const (),
    slot: u32,
    generation: u32,
    nonce: u64,
    owner: u64,
    pointer: *const u8,
    len: usize,
}
#[repr(C)]
struct Slot {
    nonce: u64,
    generation: u32,
    live: u32,
    borrows: u32,
    published: u32,
}
#[repr(C)]
struct Borrow {
    nonce: u64,
    owner: u64,
    slot: u32,
    live: u32,
}
#[repr(C)]
struct Row {
    scalar: i64,
    string: *mut u8,
    bytes: Stored,
}
#[repr(C)]
struct Staged {
    scalar: i64,
    string: *mut u8,
    bytes: Bytes,
}
#[repr(C)]
struct Rows {
    rows: *mut Row,
    len: u64,
    capacity: u64,
    generation: u64,
    domain: *mut (),
    issuer: *const (),
    invocation: u64,
}
#[repr(C)]
struct RowReceipt {
    domain: *mut (), issuer: *const (), ledger: *mut (), invocation: u64,
    pointer: *mut Row, capacity: usize, stride: usize, bytes: usize,
    funded: u32, allocated: u32, settled: u32,
}
#[repr(C)]
struct Domain {
    module: *const (),
    issuer: *mut (),
    ledger: *mut (),
    payload: *mut (),
    invocation: u64,
    vec_generation: u64,
    states: [u32; 8],
    dimensions: [usize; 7],
    slots: *mut Slot,
    views: *mut Borrow,
    backing: *mut u8,
    rows_receipt: RowReceipt,
}

/// Fixed independent view table length.
pub(super) const VIEWS: usize = 64;
/// Bound on one selected frame, matching `SPX_FUSED_FRAME_LIMIT`.
pub(super) const FRAME_LIMIT: usize = 65_536;

pub(super) fn native64() -> bool {
    size_of::<usize>() == 8 && size_of::<*const ()>() == 8
}

/// `(C type, native64 size)` for every runtime carrier, in emission order.
pub(super) fn mirror() -> [(&'static str, usize); 13] {
    [
        ("spx_fused_bytes", size_of::<Bytes>()),
        ("spx_fused_stored", size_of::<Stored>()),
        ("spx_fused_view", size_of::<View>()),
        ("spx_fused_slot", size_of::<Slot>()),
        ("spx_fused_borrow", size_of::<Borrow>()),
        ("spx_fused_row", size_of::<Row>()),
        ("spx_fused_staged", size_of::<Staged>()),
        ("spx_fused_rows", size_of::<Rows>()),
        ("spx_fused_issuer", size_of::<Issuer>()),
        ("spx_fused_ledger", size_of::<Ledger>()),
        ("spx_fused_payload", size_of::<Payload>()),
        ("spx_fused_domain", size_of::<Domain>()),
        ("spx_fused_row_receipt",size_of::<RowReceipt>()),
    ]
}

pub(super) fn row() -> usize {
    size_of::<Row>()
}

/// The heap frame: domain header, then each private carrier field. Every
/// member is 8-byte aligned with a size that is a multiple of 8, so the C
/// struct has no padding and equals this sum (asserted in the unit).
pub(super) fn frame(staged: usize, rows: usize, views: usize) -> Option<usize> {
    let bytes = size_of::<Domain>()
        .checked_add(staged.checked_mul(size_of::<Staged>())?)?
        .checked_add(rows.checked_mul(size_of::<Rows>())?)?
        .checked_add(views.checked_mul(size_of::<View>())?)?;
    (bytes <= FRAME_LIMIT).then_some(bytes)
}

/// Exact admission debit: the frame plus the lease and view tables.
pub(super) fn metadata(count: usize, frame: usize) -> Option<usize> {
    frame
        .checked_add(count.checked_mul(size_of::<Slot>())?)?
        .checked_add(VIEWS.checked_mul(size_of::<Borrow>())?)
}

/// Complete same-ledger invocation physical funding, including the exact
/// row allocation debited at its original Vec boundary.
pub(super) fn funding(count:usize,frame:usize)->Option<usize> {
    metadata(count,frame)?.checked_add(count.checked_mul(size_of::<Row>())?)
}
