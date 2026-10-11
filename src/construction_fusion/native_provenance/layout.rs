//! Legacy precursor layout of the pre-selected native runtime: one 165504-byte
//! administrative context with inline 4096-slot and 64-view tables, 24-byte
//! owners and a 40-byte row. These numbers describe that precursor contract
//! only and are kept unchanged for the legacy `Mapping`; they are not the
//! selected runtime. The selected native layout (168-byte header, exact heap
//! tables, 16-byte stored owner, 32-byte row) lives in [`selected`].
use super::*;
pub(crate) mod selected;
#[repr(C)]
struct Bytes {
    issuer: *const (),
    slot: usize,
    nonce: u64,
}
#[repr(C)]
struct View {
    issuer: *const (),
    slot: usize,
    nonce: u64,
    owner: u64,
    pointer: *const u8,
    len: usize,
}
#[repr(C)]
struct Row {
    scalar: i64,
    string: *mut (),
    bytes: Bytes,
}
#[repr(C)]
struct Slot {
    nonce: u64,
    offset: usize,
    len: usize,
    live: u32,
    borrows: u32,
    published: u32,
}
#[repr(C)]
struct Borrow {
    nonce: u64,
    owner: u64,
    live: u32,
}
#[repr(C)]
struct Issuer {
    module: *const (),
    next: u64,
    active: *mut (),
    sealed: u32,
}
#[repr(C)]
struct Context {
    module: *const (),
    issuer: *mut (),
    invocation: u64,
    sealed: u32,
    state: u32,
    failure: u32,
    pin: u32,
    count: usize,
    width: usize,
    extent: usize,
    issued: usize,
    live: usize,
    events: usize,
    cumulative: usize,
    backing: *mut u8,
    allocation_context: *mut (),
    allocate: *const (),
    release: *const (),
    slots: [Slot; 4096],
    views: [Borrow; 64],
}
pub(crate) struct TargetLayout {
    pub(crate) bytes: usize,
    pub(crate) view: usize,
    pub(crate) row: usize,
    pub(crate) vector: usize,
    pub(crate) owned: usize,
    pub(crate) view_frame_upper: usize,
    pub(crate) context: usize,
    pub(crate) issuer: usize,
}
pub(super) fn derive(descriptor: &Descriptor<'_>, views: usize) -> Option<TargetLayout> {
    for_count(descriptor.count(), views)
}
pub(super) fn for_count(count: usize, views: usize) -> Option<TargetLayout> {
    if count == 0 || count > 4096 || size_of::<usize>() != 8 {
        return None;
    }
    let owned = size_of::<Bytes>()
        .checked_add(size_of::<*mut ()>())?
        .checked_mul(count)?;
    if owned > crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES as usize {
        return None;
    }
    let vector = size_of::<Row>().checked_mul(count)?;
    let scalar = size_of::<i64>().checked_mul(count)?;
    if vector > owned.checked_add(scalar)? {
        return None;
    }
    // Each view-valued expression can introduce at most one immutable binding.
    // Future selected lowering must reserve both slots; this proof does not emit code.
    let view_frame_upper = views.checked_mul(2)?.checked_mul(size_of::<View>())?;
    Some(TargetLayout {
        bytes: size_of::<Bytes>(),
        view: size_of::<View>(),
        row: size_of::<Row>(),
        vector,
        owned,
        view_frame_upper,
        context: size_of::<Context>(),
        issuer: size_of::<Issuer>(),
    })
}
