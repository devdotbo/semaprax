//! Selected native `FusedConstructionV1` layout (authored, unexecuted).
//!
//! This is the layout of the selected native runtime
//! (`codegen/selected_fused_row/runtime.c`), kept separate from the legacy
//! precursor contract in the parent module. The invocation holds a 240-byte
//! domain header; the lease and view tables are exact heap allocations of
//! 24-byte entries made at the first original Bytes operand after the
//! separately held metadata debit; rows are 32 bytes (scalar, String pointer
//! and a 16-byte stored owner); private owner carriers are 24 bytes and views
//! 48. Every size comes from the `repr(C)` mirror whose fields match the C
//! types, compared field by field with the C compiler by the owning gate.
//!
//! The semantic owned-record charge (16 bytes per owned field per row, 131072
//! at 4096 rows) is an envelope fact. It is not `size_of::<Row>()` and is never
//! used as a physical cap; physical row storage is reported separately.
//!
//! Physical row storage is reserved on the invocation's physical ledger at the
//! original destination boundary and held as a row receipt; activation at the
//! first Bytes operand requires that receipt (see `funding`, a separate reference model; actual C admission is qualified independently).
//!
//! Layout arithmetic is not physical evidence, admission or selection. The
//! runtime receiver is [`Request::from_validated`], which reads a fresh join
//! from the private validated factory owner; it re-derives no selector and
//! grants nothing that owner has not already granted.
use crate::hir::ExpressionId;
use crate::source_verify::fused_construction::{OwnerEvent, MAX_OWNER_EVENTS};
pub(crate) mod funding;
#[cfg(test)]
mod gate;
mod mirror;
pub(crate) mod retention;
#[cfg(test)]
mod tests;

use funding::Allocation;
use retention::{Carrier, Context, Operation};

/// Smallest selected row count; 32 and fewer stay on the legacy finite route.
pub(crate) const MIN_ROWS: u32 = crate::byte_data_capacity::MAX_BYTES_COPY_SITES + 1;
/// `SPX_FUSED_ROWS`.
pub(crate) const MAX_ROWS: u32 = 4_096;
/// `SPX_FUSED_VIEWS`: the exact view table length.
pub(crate) const VIEWS: usize = 64;
/// `SPX_FUSED_EXTENT`: one backing fits one Bytes value.
pub(crate) const MAX_EXTENT: u64 = crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES;
/// Available private carrier placement under the unchanged native frame cap.
pub(crate) const MAX_PLACEMENT: usize = 65_536 - mirror::DOMAIN_BYTES;
const MAX_OPERATIONS: usize = MAX_OWNER_EVENTS + 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SelectedRefusal {
    /// Not a native64 target; the mirror and runtime asserts are native64.
    Target,
    /// Outside 33..=4096, or above the record's own Vec envelope.
    Rows,
    Width,
    /// `rows * width` overflows or exceeds one Bytes value.
    Extent,
    /// Placement items overflow or exceed the runtime's placement ceiling.
    Placement,
    /// Table, metadata or row-storage arithmetic overflowed.
    Overflow,
    /// The profile cannot retain owner context for an operation.
    Carrier(&'static str),
    /// The operation sequence is not a closed construction profile.
    Profile(&'static str),
    /// A slot or Vec generation, or the nonce budget, would be exhausted.
    Generation,
    /// The fresh join of the validated owner refused.
    Selection(crate::hir::fused_construction::Refusal),
    /// The validated owner's capacity facts disagree with its fresh join.
    Capacity,
    /// The destination identity is empty or not the layout's destination.
    Destination,
    /// The physical ledger refused the exact row storage at the destination.
    RowsUnfunded,
    /// Activation was presented no matching live row receipt.
    RowReceipt,
    /// Activation found another domain active on the held issuer.
    IssuerBusy,
    /// The held issuer lacks headroom for the invocation and every issue.
    NonceHeadroom,
    /// The physical ledger refused the exact metadata debit.
    MetadataUnfunded,
    /// The ordinary event or cumulative counters refused the backing.
    OrdinaryUnfunded,
}

/// Frame items the native lowering places in the invocation, priced at their
/// actual target sizes: staged receipts, row Vec headers and views.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) struct Placement {
    pub(crate) staged: usize,
    pub(crate) rows: usize,
    pub(crate) views: usize,
}

impl Placement {
    /// `SPX_FUSED_SELECTED_PLACEMENT`, with checked arithmetic.
    pub(crate) fn bytes(&self) -> Option<usize> {
        self.staged
            .checked_mul(mirror::STAGED_BYTES)?
            .checked_add(self.rows.checked_mul(mirror::ROWS_BYTES)?)?
            .checked_add(self.views.checked_mul(mirror::VIEW_BYTES)?)
    }
}

/// What the selected native layout is derived from.
pub(crate) struct Request {
    rows: u32,
    width: u64,
    /// Canonical identity of the destination `vec_with_capacity` expression.
    destination: ExpressionId,
    placement: Placement,
    operations: [Option<Operation>; MAX_OPERATIONS],
}

/// Passive carrier profile from authenticated source owner events: push first,
/// then each move, sort and borrow in source order, then the passive Vec
/// settlement marker. This is proof data for the carrier-retention model; it
/// is never cleanup-plan permission. Engines settle through the full canonical
/// cleanup plan of the validated program.
fn passive_profile(
    joined: &crate::hir::fused_construction::join::Joined<'_>,
    placement: Placement,
) -> Result<Request, SelectedRefusal> {
    let mut operations = [None; MAX_OPERATIONS];
    operations[0] = Some(Operation::Push);
    let mut len = 1;
    for event in joined.proof().owner_events() {
        let operation = match event {
            OwnerEvent::Move(_) => Operation::VecMove,
            OwnerEvent::Sort(_) => Operation::VecSort,
            OwnerEvent::Borrow(_) => Operation::RowsBorrow,
        };
        *operations
            .get_mut(len)
            .ok_or(SelectedRefusal::Profile("too many owner events"))? = Some(operation);
        len += 1;
    }
    *operations
        .get_mut(len)
        .ok_or(SelectedRefusal::Profile("too many owner events"))? =
        Some(Operation::PassiveVecSettlement);
    let selection = joined.selection();
    Ok(Request {
        rows: selection.rows(),
        width: selection.width(),
        destination: selection.allocation().id.clone(),
        placement,
        operations,
    })
}

impl Request {
    /// The runtime receiver: read a fresh join from the private validated
    /// factory owner and require its capacity facts to agree with that join.
    /// `placement` is the native lowering's own frame item census. Nothing here
    /// activates; no production route calls this by default.
    pub(crate) fn from_validated(
        owner: &crate::hir::selected_fused_candidate::ValidatedFusedSelection<'_>,
        placement: Placement,
    ) -> Result<Self, SelectedRefusal> {
        let capacity = owner.capacity();
        owner
            .with_joined(|joined| {
                let selection = joined.selection();
                if capacity.rows != selection.rows()
                    || capacity.width != selection.width()
                    || capacity.extent != selection.extent()
                    || capacity.fused_backing_events != 1
                {
                    return Err(SelectedRefusal::Capacity);
                }
                passive_profile(joined, placement)
            })
            .map_err(SelectedRefusal::Selection)?
    }

    /// Proof-only receiver over an unvalidated join, for owning tests. It is
    /// never runtime authority.
    #[cfg(test)]
    pub(super) fn passive_from_joined(
        joined: &crate::hir::fused_construction::join::Joined<'_>,
        placement: Placement,
    ) -> Result<Self, SelectedRefusal> {
        passive_profile(joined, placement)
    }

    #[cfg(test)]
    pub(super) fn for_test(
        rows: u32,
        width: u64,
        placement: Placement,
        operations: &[Operation],
    ) -> Option<Self> {
        let mut table = [None; MAX_OPERATIONS];
        for (slot, operation) in table.iter_mut().zip(operations) {
            *slot = Some(*operation);
        }
        (operations.len() <= MAX_OPERATIONS).then_some(Self {
            rows,
            width,
            destination: test_destination(),
            placement,
            operations: table,
        })
    }

    fn operations(&self) -> impl Iterator<Item = Operation> + '_ {
        self.operations.iter().map_while(|operation| *operation)
    }
}

/// The canonical destination identity of the fixture's `app.main`.
#[cfg(test)]
pub(super) fn test_destination() -> ExpressionId {
    let main =
        crate::hir::FunctionExecutionId::Monomorphic(crate::hir::DeclarationId::new("app.main"));
    ExpressionId::new(&main, "body.s0.value")
}

/// The selected native layout for one request on this target.
#[derive(Debug, Eq, PartialEq)]
pub(crate) struct SelectedNativeLayout {
    rows: u32,
    width: u64,
    destination: ExpressionId,
    extent: usize,
    header: usize,
    slot_table: usize,
    view_table: usize,
    placement: usize,
    metadata: usize,
    row_storage: usize,
    semantic_owned: u64,
    semantic_scalar: u64,
    max_slot_generation: u32,
    final_vec_generation: u64,
    nonces_through_completion: u64,
}

impl SelectedNativeLayout {
    pub(crate) fn rows(&self) -> u32 {
        self.rows
    }
    pub(crate) fn width(&self) -> u64 {
        self.width
    }
    pub(crate) fn extent(&self) -> usize {
        self.extent
    }
    /// `sizeof(spx_fused_domain)`, held by the invocation, debited with tables.
    pub(crate) fn header(&self) -> usize {
        self.header
    }
    pub(crate) fn slot_table(&self) -> usize {
        self.slot_table
    }
    pub(crate) fn view_table(&self) -> usize {
        self.view_table
    }
    pub(crate) fn placement(&self) -> usize {
        self.placement
    }
    /// `spx_fused_metadata_bytes(count, header + placement)`.
    pub(crate) fn metadata(&self) -> usize {
        self.metadata
    }
    /// Physical row Vec storage, `count * sizeof(spx_fused_row)`, reserved on
    /// the physical ledger at the destination boundary.
    pub(crate) fn row_storage(&self) -> usize {
        self.row_storage
    }
    /// Semantic owned-field charge under the ordinary record Vec envelope. It
    /// is an envelope fact and never funds or substitutes for row storage.
    pub(crate) fn semantic_owned(&self) -> u64 {
        self.semantic_owned
    }
    pub(crate) fn semantic_scalar(&self) -> u64 {
        self.semantic_scalar
    }
    /// Highest 32-bit slot generation the profile reaches (issue is 1).
    pub(crate) fn max_slot_generation(&self) -> u32 {
        self.max_slot_generation
    }
    /// The row Vec header generation after the profile's last renewal.
    pub(crate) fn final_vec_generation(&self) -> u64 {
        self.final_vec_generation
    }
    /// Invocation plus one issue and one push renewal per row. View nonces are
    /// minted per loan and refuse at the never-wrapping mint.
    pub(crate) fn nonces_through_completion(&self) -> u64 {
        self.nonces_through_completion
    }
    /// The destination whose row storage a receipt must name.
    pub(crate) fn destination(&self) -> &ExpressionId {
        &self.destination
    }
    /// The intended protocol's heap allocations in source order, with account
    /// and boundary. Reference model only until the C runtime funds row storage.
    pub(crate) fn plan(&self) -> [Allocation; 4] {
        funding::plan(self)
    }
    pub(crate) fn owner_carrier() -> usize {
        mirror::OWNER_BYTES
    }
    pub(crate) fn stored_owner() -> usize {
        mirror::STORED_BYTES
    }
    pub(crate) fn view_carrier() -> usize {
        mirror::VIEW_BYTES
    }
    pub(crate) fn row_stride() -> usize {
        mirror::ROW_BYTES
    }
}

/// `spx_fused_metadata_bytes`: header, exact tables and placement.
pub(crate) fn metadata_bytes(rows: usize, placement: usize) -> Option<usize> {
    if rows == 0 || rows > MAX_ROWS as usize || placement > MAX_PLACEMENT {
        return None;
    }
    mirror::DOMAIN_BYTES
        .checked_add(rows.checked_mul(mirror::SLOT_BYTES)?)?
        .checked_add(VIEWS.checked_mul(mirror::BORROW_BYTES)?)?
        .checked_add(placement)
}

/// Prove every operation keeps its owner context and count renewals.
fn retain(request: &Request) -> Result<(u32, u64), SelectedRefusal> {
    let mut operations = request.operations();
    if operations.next() != Some(Operation::Push) {
        return Err(SelectedRefusal::Profile(
            "profile does not begin at the push commit",
        ));
    }
    let mut held = Carrier::Stored;
    let mut slot_generation = 1u32.checked_add(retention::slot_renewals(Operation::Push));
    let mut vec_generation = 1u64.checked_add(u64::from(request.rows));
    let mut settled = false;
    for operation in operations {
        if settled {
            return Err(SelectedRefusal::Profile("operation after Vec settlement"));
        }
        let transition = retention::transition(operation).ok_or(SelectedRefusal::Carrier(
            "private carrier crossing or stored escape",
        ))?;
        // A receipt holds a full owner carrier with the held domain's issuer.
        let accepts = transition.input == held
            || (held == Carrier::Receipt && transition.input == Carrier::Owner);
        if !accepts {
            return Err(SelectedRefusal::Carrier(
                "operation does not receive its held carrier",
            ));
        }
        if transition.context == Context::Settled {
            settled = true;
        } else if !matches!(
            operation,
            Operation::RowsBorrow | Operation::RowsLen | Operation::ChargedClone
        ) {
            // Loans, header reads and charged clones leave the child in place;
            // a clone's ordinary copy is outside the lease domain.
            held = transition.output;
        }
        slot_generation = slot_generation
            .and_then(|value| value.checked_add(retention::slot_renewals(operation)));
        if transition.renews_vec {
            vec_generation = vec_generation.and_then(|value| value.checked_add(1));
        }
    }
    if !settled {
        return Err(SelectedRefusal::Profile(
            "profile never settles the row Vec",
        ));
    }
    match (slot_generation, vec_generation) {
        (Some(slot), Some(vec)) => Ok((slot, vec)),
        _ => Err(SelectedRefusal::Generation),
    }
}

/// Derive the selected native layout. Every quantity is checked; a profile
/// that cannot prove owner-context retention refuses.
pub(crate) fn derive(request: &Request) -> Result<SelectedNativeLayout, SelectedRefusal> {
    if !cfg!(target_pointer_width = "64") {
        return Err(SelectedRefusal::Target);
    }
    if request.rows < MIN_ROWS || request.rows > MAX_ROWS {
        return Err(SelectedRefusal::Rows);
    }
    if request.width == 0 {
        return Err(SelectedRefusal::Width);
    }
    let extent = u64::from(request.rows)
        .checked_mul(request.width)
        .filter(|extent| *extent <= MAX_EXTENT)
        .and_then(|extent| usize::try_from(extent).ok())
        .ok_or(SelectedRefusal::Extent)?;
    let rows = request.rows as usize;
    let placement = request
        .placement
        .bytes()
        .filter(|bytes| *bytes <= MAX_PLACEMENT)
        .ok_or(SelectedRefusal::Placement)?;
    let slot_table = rows
        .checked_mul(mirror::SLOT_BYTES)
        .ok_or(SelectedRefusal::Overflow)?;
    let view_table = VIEWS
        .checked_mul(mirror::BORROW_BYTES)
        .ok_or(SelectedRefusal::Overflow)?;
    let metadata = metadata_bytes(rows, placement).ok_or(SelectedRefusal::Overflow)?;
    let row_storage = rows
        .checked_mul(mirror::ROW_BYTES)
        .ok_or(SelectedRefusal::Overflow)?;
    // One scalar and two owned fields: the existing record Vec envelopes.
    let semantic_owned = u64::from(request.rows)
        .checked_mul(2 * crate::vec_ops::OWNED_PAYLOAD_BYTES_PER_ELEMENT)
        .filter(|charge| *charge <= crate::vec_ops::MAX_OWNED_PAYLOAD_BYTES)
        .ok_or(SelectedRefusal::Rows)?;
    let semantic_scalar = u64::from(request.rows)
        .checked_mul(8)
        .filter(|_| u64::from(request.rows) <= crate::vec_ops::MAX_CAPACITY)
        .ok_or(SelectedRefusal::Rows)?;
    let (max_slot_generation, final_vec_generation) = retain(request)?;
    let nonces_through_completion = u64::from(request.rows)
        .checked_mul(2)
        .and_then(|nonces| nonces.checked_add(1))
        .ok_or(SelectedRefusal::Generation)?;
    if request.destination.as_str().is_empty() {
        return Err(SelectedRefusal::Destination);
    }
    Ok(SelectedNativeLayout {
        rows: request.rows,
        width: request.width,
        destination: request.destination.clone(),
        extent,
        header: mirror::DOMAIN_BYTES,
        slot_table,
        view_table,
        placement,
        metadata,
        row_storage,
        semantic_owned,
        semantic_scalar,
        max_slot_generation,
        final_vec_generation,
        nonces_through_completion,
    })
}
