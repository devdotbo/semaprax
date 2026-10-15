//! `repr(C)` mirror of the selected native runtime's private carriers and
//! tables (`codegen/selected_fused_row/runtime.c`). Every field keeps the C
//! field's exact type: `uint32_t`/`uint64_t` map to `u32`/`u64`, `size_t` to
//! `usize`, and pointers to raw pointers. Nothing here is a generic usize
//! guess. The mirror is measured, never instantiated or read; the owning
//! cross-source gate compares every size, alignment and offset with the C
//! compiler's own `sizeof`/`_Alignof`/`offsetof` for the same target.
#![allow(
    dead_code,
    reason = "layout mirror of the C runtime: fields are measured, never read"
)]
use std::mem::{align_of, offset_of, size_of};

/// `spx_fused_issuer`: caller-held, never reset, nonces never wrap.
#[repr(C)]
pub(super) struct Issuer {
    module: *const (),
    next: u64,
    active: *const (),
    sealed: u32,
}
/// `spx_fused_ledger`: separately held metadata account, no refund.
#[repr(C)]
pub(super) struct Ledger {
    limit: usize,
    debited: usize,
    held_rows: usize,
    sealed: u32,
    failed: u32,
}
/// `spx_fused_payload`: caller-held ordinary event and payload ledger.
#[repr(C)]
pub(super) struct Payload {
    events: usize,
    cumulative: usize,
    held_events: usize,
    held_bytes: usize,
    sealed: u32,
    failed: u32,
}
/// `spx_fused_copy`: independently allocated ordinary clone carrier.
#[repr(C)]
pub(super) struct Copy {
    ptr: *mut u8,
    len: u64,
}
/// `spx_fused_row_receipt`: address-bound, destination-funded receipt.
#[repr(C)]
pub(super) struct RowReceipt {
    domain: *mut Domain,
    issuer: *const Issuer,
    ledger: *mut Ledger,
    invocation: u64,
    pointer: *mut Row,
    capacity: usize,
    stride: usize,
    bytes: usize,
    funded: u32,
    allocated: u32,
    settled: u32,
}
/// `spx_fused_bytes`: the 24-byte private owner carrier (staged, popped,
/// moved). It carries its issuer explicitly.
#[repr(C)]
pub(super) struct Bytes {
    issuer: *const (),
    slot: u32,
    generation: u32,
    nonce: u64,
}
/// `spx_fused_stored`: the 16-byte in-row owner. It carries no issuer; its
/// context is the held row Vec header's domain.
#[repr(C)]
pub(super) struct Stored {
    slot: u32,
    generation: u32,
    nonce: u64,
}
/// `spx_fused_view`: the 48-byte private readonly view carrier.
#[repr(C)]
pub(super) struct View {
    issuer: *const (),
    slot: u32,
    generation: u32,
    nonce: u64,
    owner: u64,
    ptr: *const u8,
    len: usize,
}
/// `spx_fused_slot`: one exact lease-table entry.
#[repr(C)]
pub(super) struct Slot {
    nonce: u64,
    generation: u32,
    live: u32,
    borrows: u32,
    published: u32,
}
/// `spx_fused_borrow`: one exact view-table entry.
#[repr(C)]
pub(super) struct Borrow {
    nonce: u64,
    owner: u64,
    slot: u32,
    live: u32,
}
/// `spx_fused_row`: the stored row (scalar, String pointer, stored owner).
#[repr(C)]
pub(super) struct Row {
    scalar: i64,
    string: *mut u8,
    bytes: Stored,
}
/// `spx_fused_staged`: a staged or popped row receipt holding a full owner.
#[repr(C)]
pub(super) struct Staged {
    scalar: i64,
    string: *mut u8,
    bytes: Bytes,
}
/// `spx_fused_rows`: the row Vec header bound to one held domain/generation.
#[repr(C)]
pub(super) struct Rows {
    rows: *mut Row,
    len: u64,
    capacity: u64,
    generation: u64,
    domain: *mut Domain,
    issuer: *const Issuer,
    invocation: u64,
}
/// `spx_fused_domain`: the small invocation-held header. Tables and backing
/// are exact heap allocations referenced by pointer.
#[repr(C)]
pub(super) struct Domain {
    module: *const (),
    issuer: *mut Issuer,
    ledger: *mut Ledger,
    payload: *mut Payload,
    invocation: u64,
    vec_generation: u64,
    sealed: u32,
    state: u32,
    failure: u32,
    pin: u32,
    vec_live: u32,
    activated: u32,
    held: u32,
    consumed: u32,
    count: usize,
    width: usize,
    extent: usize,
    metadata: usize,
    issued: usize,
    live: usize,
    loans: usize,
    slots: *mut Slot,
    views: *mut Borrow,
    backing: *mut u8,
    rows_receipt: RowReceipt,
}

/// One measured C struct: C name, Rust size and alignment.
pub(super) struct Shape {
    pub(super) name: &'static str,
    pub(super) size: usize,
    pub(super) align: usize,
}
/// One measured field: C struct, C field, Rust offset and size.
pub(super) struct Field {
    pub(super) owner: &'static str,
    pub(super) name: &'static str,
    pub(super) offset: usize,
    pub(super) size: usize,
}

macro_rules! shape {
    ($name:literal, $ty:ty) => {
        Shape {
            name: $name,
            size: size_of::<$ty>(),
            align: align_of::<$ty>(),
        }
    };
}
macro_rules! fields {
    ($owner:literal, $ty:ty, $($field:ident: $field_ty:ty),+ $(,)?) => {
        [$(Field {
            owner: $owner,
            name: stringify!($field),
            offset: offset_of!($ty, $field),
            size: size_of::<$field_ty>(),
        }),+]
    };
}

pub(super) const SHAPES: [Shape; 14] = [
    shape!("spx_fused_issuer", Issuer),
    shape!("spx_fused_ledger", Ledger),
    shape!("spx_fused_payload", Payload),
    shape!("spx_fused_copy", Copy),
    shape!("spx_fused_row_receipt", RowReceipt),
    shape!("spx_fused_bytes", Bytes),
    shape!("spx_fused_stored", Stored),
    shape!("spx_fused_view", View),
    shape!("spx_fused_slot", Slot),
    shape!("spx_fused_borrow", Borrow),
    shape!("spx_fused_row", Row),
    shape!("spx_fused_staged", Staged),
    shape!("spx_fused_rows", Rows),
    shape!("spx_fused_domain", Domain),
];

pub(super) const ISSUER: [Field; 4] = fields!("spx_fused_issuer", Issuer,
    module: *const (), next: u64, active: *const (), sealed: u32);
pub(super) const LEDGER: [Field; 5] = fields!("spx_fused_ledger", Ledger,
    limit: usize, debited: usize, held_rows: usize, sealed: u32, failed: u32);
pub(super) const BYTES: [Field; 4] = fields!("spx_fused_bytes", Bytes,
    issuer: *const (), slot: u32, generation: u32, nonce: u64);
pub(super) const STORED: [Field; 3] = fields!("spx_fused_stored", Stored,
    slot: u32, generation: u32, nonce: u64);
pub(super) const VIEW: [Field; 7] = fields!("spx_fused_view", View,
    issuer: *const (), slot: u32, generation: u32, nonce: u64, owner: u64,
    ptr: *const u8, len: usize);
pub(super) const SLOT: [Field; 5] = fields!("spx_fused_slot", Slot,
    nonce: u64, generation: u32, live: u32, borrows: u32, published: u32);
pub(super) const BORROW: [Field; 4] = fields!("spx_fused_borrow", Borrow,
    nonce: u64, owner: u64, slot: u32, live: u32);
pub(super) const ROW: [Field; 3] = fields!("spx_fused_row", Row,
    scalar: i64, string: *mut u8, bytes: Stored);
pub(super) const STAGED: [Field; 3] = fields!("spx_fused_staged", Staged,
    scalar: i64, string: *mut u8, bytes: Bytes);
pub(super) const ROWS: [Field; 7] = fields!("spx_fused_rows", Rows,
    rows: *mut Row, len: u64, capacity: u64, generation: u64, domain: *mut Domain, issuer: *const Issuer, invocation: u64);
pub(super) const PAYLOAD: [Field; 6] = fields!("spx_fused_payload", Payload,
    events: usize, cumulative: usize, held_events: usize, held_bytes: usize, sealed: u32, failed: u32);
pub(super) const COPY: [Field; 2] = fields!("spx_fused_copy", Copy, ptr: *mut u8, len: u64);
pub(super) const RECEIPT: [Field; 11] = fields!("spx_fused_row_receipt", RowReceipt,
    domain: *mut Domain, issuer: *const Issuer, ledger: *mut Ledger, invocation: u64,
    pointer: *mut Row, capacity: usize, stride: usize, bytes: usize,
    funded: u32, allocated: u32, settled: u32);
pub(super) const DOMAIN: [Field; 25] = fields!("spx_fused_domain", Domain,
    module: *const (), issuer: *mut Issuer, ledger: *mut Ledger, payload: *mut Payload,
    invocation: u64, vec_generation: u64,
    sealed: u32, state: u32, failure: u32, pin: u32, vec_live: u32, activated: u32,
    held: u32, consumed: u32,
    count: usize, width: usize, extent: usize, metadata: usize, issued: usize, live: usize, loans: usize,
    slots: *mut Slot, views: *mut Borrow, backing: *mut u8, rows_receipt: RowReceipt);

/// Every measured field table, for the cross-source gate.
pub(super) fn fields() -> [&'static [Field]; 14] {
    [
        &ISSUER, &LEDGER, &PAYLOAD, &COPY, &RECEIPT, &BYTES, &STORED, &VIEW, &SLOT, &BORROW, &ROW,
        &STAGED, &ROWS, &DOMAIN,
    ]
}

pub(super) const DOMAIN_BYTES: usize = size_of::<Domain>();
pub(super) const SLOT_BYTES: usize = size_of::<Slot>();
pub(super) const BORROW_BYTES: usize = size_of::<Borrow>();
pub(super) const OWNER_BYTES: usize = size_of::<Bytes>();
pub(super) const STORED_BYTES: usize = size_of::<Stored>();
pub(super) const VIEW_BYTES: usize = size_of::<View>();
pub(super) const ROW_BYTES: usize = size_of::<Row>();
pub(super) const STAGED_BYTES: usize = size_of::<Staged>();
pub(super) const ROWS_BYTES: usize = size_of::<Rows>();

// The runtime's own native64 `_Static_assert` values, restated independently.
// A mismatch fails compilation on a 64-bit target before any probe runs.
#[cfg(target_pointer_width = "64")]
const _: () = {
    assert!(size_of::<Bytes>() == 24);
    assert!(size_of::<Stored>() == 16);
    assert!(size_of::<View>() == 48);
    assert!(size_of::<Slot>() == 24);
    assert!(size_of::<Borrow>() == 24);
    assert!(size_of::<Row>() == 32);
    assert!(size_of::<Staged>() == 40);
    assert!(size_of::<Rows>() == 56);
    assert!(size_of::<Issuer>() == 32);
    assert!(size_of::<Ledger>() == 32);
    assert!(size_of::<Domain>() == 240);
    assert!(size_of::<Payload>() == 40);
    assert!(size_of::<RowReceipt>() == 80);
    assert!(size_of::<Copy>() == 16);
    assert!(offset_of!(Row, bytes) == 16);
    assert!(offset_of!(Staged, bytes) == 16);
    assert!(offset_of!(Domain, count) == 80);
    assert!(offset_of!(Domain, slots) == 136);
};
