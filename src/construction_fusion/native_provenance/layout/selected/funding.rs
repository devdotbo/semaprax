//! Reference funding model for the selected native layout (model only).
//!
//! One physical ledger, held by the invocation, funds both physical
//! inventories in source order:
//!
//! 1. At the original destination boundary (`vec_with_capacity<Row>(N)`,
//!    before the loop), the exact row storage `N * sizeof(spx_fused_row)` is
//!    reserved and an unforgeable [`RowReceipt`] is held. A refusal fails at
//!    that boundary; it is never moved to the first Bytes operand.
//! 2. At the first original Bytes operand, activation requires that live
//!    receipt, claims the held issuer and debits the header, both exact tables
//!    and frame placement on the same ledger, then the ordinary event and
//!    payload counters before the backing.
//! 3. Retiring the receipt (when the engine's canonical cleanup plan settles
//!    the row Vec) refunds nothing; a refused ledger stays refused.
//!
//! The semantic owned-record Vec envelope and the ordinary Bytes event and
//! payload budgets are never substituted for the physical row reservation.
//!
//! This is a pure reference protocol, not the actual native admission order:
//! native administrative admission debits metadata and holds row/payload funding
//! before effects, converts the row hold at the original Vec boundary and
//! consumes the payload hold at the first Bytes operand. The cross-source gate
//! drives that actual runtime separately; this model grants no runtime authority.
use super::{mirror, SelectedNativeLayout, SelectedRefusal};
use crate::hir::ExpressionId;
use std::cell::Cell;
use std::sync::atomic::{AtomicU64, Ordering};

/// Who pays for an allocation in the reference model.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Account {
    /// The invocation's physical ledger: row storage at the destination, then
    /// header, exact tables and placement at the first Bytes operand.
    PhysicalLedger,
    /// The ordinary event and cumulative payload counters, debited before the
    /// single zeroed backing and never refunded.
    OrdinaryEvents,
}

/// The original source boundary at which the allocation and its failure occur.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Boundary {
    /// `vec_with_capacity<Row>(N)`, before the construction loop.
    DestinationAllocation,
    /// The first original Bytes operand, after the preceding String operand.
    FirstBytesOperand,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Kind {
    RowStorage,
    SlotTable,
    ViewTable,
    Backing,
}

/// One heap allocation of the intended protocol, in source order.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Allocation {
    pub(crate) kind: Kind,
    pub(crate) bytes: usize,
    pub(crate) account: Account,
    pub(crate) boundary: Boundary,
}

/// Intended protocol order: row storage at the destination; then, at the
/// first Bytes operand, the metadata debit, both exact tables, the ordinary
/// debit and the single backing.
pub(super) fn plan(layout: &SelectedNativeLayout) -> [Allocation; 4] {
    [
        Allocation {
            kind: Kind::RowStorage,
            bytes: layout.row_storage,
            account: Account::PhysicalLedger,
            boundary: Boundary::DestinationAllocation,
        },
        Allocation {
            kind: Kind::SlotTable,
            bytes: layout.slot_table,
            account: Account::PhysicalLedger,
            boundary: Boundary::FirstBytesOperand,
        },
        Allocation {
            kind: Kind::ViewTable,
            bytes: layout.view_table,
            account: Account::PhysicalLedger,
            boundary: Boundary::FirstBytesOperand,
        },
        Allocation {
            kind: Kind::Backing,
            bytes: layout.extent,
            account: Account::OrdinaryEvents,
            boundary: Boundary::FirstBytesOperand,
        },
    ]
}

/// Process-unique, never-zero identities for ledgers and issuers, so a receipt
/// from another account or issuer cannot be presented.
static IDENTITIES: AtomicU64 = AtomicU64::new(1);
fn next_identity(counter: &AtomicU64) -> Option<u64> {
    counter.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
        value.checked_add(1).filter(|_| value != 0)
    }).ok()
}
fn identity() -> Option<u64> {
    next_identity(&IDENTITIES)
}
#[cfg(test)]
pub(super) fn identity_exhaustion_refuses_without_wrap() {
    let counter = AtomicU64::new(u64::MAX - 1);
    assert_eq!(next_identity(&counter), Some(u64::MAX - 1));
    assert_eq!(next_identity(&counter), None);
    assert_eq!(next_identity(&counter), None);
    assert_eq!(counter.load(Ordering::Relaxed), u64::MAX);
}

/// Mirror of `spx_fused_ledger`: monotonic debits, first refusal is sticky,
/// no refund and no reset.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct Ledger {
    account: u64,
    limit: usize,
    debited: usize,
    failed: bool,
}

impl Ledger {
    pub(crate) fn new(limit: usize) -> Self {
        let account = identity();
        Self {
            account: account.unwrap_or(0),
            limit,
            debited: 0,
            failed: account.is_none(),
        }
    }
    pub(crate) fn debited(&self) -> usize {
        self.debited
    }
    pub(crate) fn failed(&self) -> bool {
        self.failed
    }
    fn debit(&mut self, bytes: usize) -> bool {
        if self.failed {
            return false;
        }
        match self.limit.checked_sub(self.debited) {
            Some(room) if bytes <= room => {
                self.debited += bytes;
                true
            }
            _ => {
                self.failed = true;
                false
            }
        }
    }
}

/// Mirror of the caller-held `spx_fused_issuer`: one active domain at a time,
/// strictly increasing nonces that never wrap or reset.
#[derive(Debug)]
pub(crate) struct HeldIssuer {
    identity: u64,
    next: u64,
    active: Option<u64>,
}

impl HeldIssuer {
    pub(crate) fn new() -> Self {
        let identity = identity();
        Self {
            identity: identity.unwrap_or(0),
            next: if identity.is_some() { 1 } else { 0 },
            active: None,
        }
    }
    #[cfg(test)]
    pub(super) fn exhausted_for_test(next: u64) -> Self {
        Self {
            identity: identity().expect("test identity headroom"),
            next,
            active: None,
        }
    }
    pub(crate) fn active(&self) -> Option<u64> {
        self.active
    }
}

/// Ordinary per-invocation counters (32 events, 2 MiB cumulative). They fund
/// the backing only; they never fund row storage or metadata.
#[derive(Debug, Default)]
pub(crate) struct Ordinary {
    events: u32,
    cumulative: u64,
}

impl Ordinary {
    pub(crate) fn events(&self) -> u32 {
        self.events
    }
    pub(crate) fn cumulative(&self) -> u64 {
        self.cumulative
    }
    #[cfg(test)]
    pub(super) fn spent_for_test(events: u32, cumulative: u64) -> Self {
        Self { events, cumulative }
    }
}

/// The held invocation owner: its issuer, physical ledger and ordinary
/// counters. Only this owner reserves rows and activates.
pub(crate) struct HeldInvocation<'h> {
    pub(crate) issuer: &'h mut HeldIssuer,
    pub(crate) ledger: &'h mut Ledger,
    pub(crate) ordinary: &'h mut Ordinary,
}

/// Held proof that the exact row storage of one destination was debited from
/// one physical ledger. It has no public constructor, is neither `Clone` nor
/// `Copy`, and is consumed by [`settle_row_receipt`].
#[derive(Debug)]
pub(crate) struct RowReceipt {
    account: u64,
    issuer: u64,
    destination: ExpressionId,
    rows: u32,
    stride: usize,
    bytes: usize,
    nonce: u64,
    activated: Cell<bool>,
}

impl RowReceipt {
    pub(crate) fn bytes(&self) -> usize {
        self.bytes
    }
    pub(crate) fn nonce(&self) -> u64 {
        self.nonce
    }
}

/// Reserve the exact row storage at the original destination boundary. The
/// ordinary counters and the semantic Vec envelope are not consulted.
pub(crate) fn reserve_rows(
    layout: &SelectedNativeLayout,
    held: &mut HeldInvocation<'_>,
    destination: &ExpressionId,
) -> Result<RowReceipt, SelectedRefusal> {
    if *destination != layout.destination || destination.as_str().is_empty() {
        return Err(SelectedRefusal::Destination);
    }
    if held.issuer.next == 0 || held.issuer.next == u64::MAX {
        return Err(SelectedRefusal::NonceHeadroom);
    }
    if !held.ledger.debit(layout.row_storage) {
        return Err(SelectedRefusal::RowsUnfunded);
    }
    let nonce = held.issuer.next;
    held.issuer.next += 1;
    Ok(RowReceipt {
        account: held.ledger.account,
        issuer: held.issuer.identity,
        destination: destination.clone(),
        rows: layout.rows,
        stride: mirror::ROW_BYTES,
        bytes: layout.row_storage,
        nonce,
        activated: Cell::new(false),
    })
}

/// The receipt's debit has been retired by canonical cleanup. Nothing is
/// refunded: the ledger keeps every debit.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SettledRows {
    pub(crate) bytes: usize,
}

/// Retire a row receipt when the engine's canonical cleanup plan settles the
/// row Vec. This model grants no cleanup permission of its own.
pub(crate) fn settle_row_receipt(receipt: RowReceipt) -> SettledRows {
    SettledRows {
        bytes: receipt.bytes,
    }
}

/// A reference-model activation: the claimed invocation, the row receipt's
/// debit it relied on, and the first-Bytes allocations whose debits it made.
#[derive(Debug)]
pub(crate) struct Activation {
    pub(crate) invocation: u64,
    pub(crate) row_storage: usize,
    pub(crate) metadata: usize,
    pub(crate) first_bytes: [Allocation; 3],
}

/// Mirror of the intended activation: require the held row receipt, claim the
/// held issuer, debit the exact metadata on the same ledger, then the ordinary
/// counters. A receipt or metadata refusal claims nothing; an ordinary refusal
/// leaves the claimed domain to its finish.
pub(crate) fn activate(
    layout: &SelectedNativeLayout,
    held: &mut HeldInvocation<'_>,
    receipt: &RowReceipt,
) -> Result<Activation, SelectedRefusal> {
    if receipt.account != held.ledger.account
        || receipt.issuer != held.issuer.identity
        || receipt.destination != layout.destination
        || receipt.rows != layout.rows
        || receipt.stride != mirror::ROW_BYTES
        || receipt.bytes != layout.row_storage
        || receipt.nonce == 0
        || receipt.nonce >= held.issuer.next
    {
        return Err(SelectedRefusal::RowReceipt);
    }
    if held.issuer.active.is_some() {
        return Err(SelectedRefusal::IssuerBusy);
    }
    if receipt.activated.get() {
        return Err(SelectedRefusal::RowReceipt);
    }
    // The runtime requires headroom for the invocation and every issue nonce;
    // later renewals refuse at the never-wrapping mint.
    let headroom = u64::MAX
        .checked_sub(u64::from(layout.rows))
        .and_then(|value| value.checked_sub(1))
        .ok_or(SelectedRefusal::NonceHeadroom)?;
    if held.issuer.next == 0 || held.issuer.next >= headroom {
        return Err(SelectedRefusal::NonceHeadroom);
    }
    if !held.ledger.debit(layout.metadata) {
        return Err(SelectedRefusal::MetadataUnfunded);
    }
    receipt.activated.set(true);
    let invocation = held.issuer.next;
    held.issuer.next += 1;
    held.issuer.active = Some(invocation);
    let [_, slots, views, backing] = plan(layout);
    let room = crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES
        .checked_sub(held.ordinary.cumulative);
    if held.ordinary.events >= crate::byte_data_capacity::MAX_BYTES_COPY_SITES
        || room.is_none_or(|room| layout.extent as u64 > room)
    {
        return Err(SelectedRefusal::OrdinaryUnfunded);
    }
    held.ordinary.events += 1;
    held.ordinary.cumulative += layout.extent as u64;
    Ok(Activation {
        invocation,
        row_storage: receipt.bytes,
        metadata: layout.metadata,
        first_bytes: [slots, views, backing],
    })
}

/// Mirror of `spx_fused_finish` for the issuer claim, including a failed
/// activation that already claimed it. Debits are never refunded.
pub(crate) fn finish(held: &mut HeldInvocation<'_>) -> Option<u64> {
    held.issuer.active.take()
}
