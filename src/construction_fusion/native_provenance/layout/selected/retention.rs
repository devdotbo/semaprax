//! Owner-context retention for the selected native carriers.
//!
//! The 16-byte stored owner has no issuer field. Its issuer, nonce and exact
//! generation are authenticated only through the held row Vec header, whose
//! `domain` and `generation` must equal the held domain's current values. A
//! popped or staged row is a 40-byte receipt whose 24-byte owner carries the
//! held domain's issuer and a renewed nonce and generation. This module refuses
//! any operation whose output carrier would lose that context, and every
//! private carrier crossing toward the frozen public 16-byte Bytes ABI.

/// One private owner representation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Carrier {
    /// `spx_fused_bytes`, self-describing (issuer, slot, generation, nonce).
    Owner,
    /// `spx_fused_stored`, valid only inside a held row Vec header.
    Stored,
    /// `spx_fused_staged`, a staged or popped receipt holding an `Owner`.
    Receipt,
    /// `spx_fused_view`, a readonly loan with its own nonce.
    View,
    /// `spx_fused_copy`, a fresh ordinary owner from a charged deep clone.
    OrdinaryCopy,
}

/// Where an output carrier's issuer context comes from.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Context {
    /// The carrier's own issuer pointer, checked against the held domain.
    CarriedIssuer,
    /// The held row Vec header: `rows.domain == domain` and
    /// `rows.generation == domain.vec_generation`.
    HeldRowsHeader,
    /// No private context: an ordinary owner outside the lease domain.
    Ordinary,
    /// The carrier settles; nothing is produced.
    Settled,
}

/// Owner operations a selected profile may use.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Operation {
    /// Staged receipt to stored row at the atomic push commit.
    Push,
    /// Immutable whole-Vec transfer; the prior header retires by generation.
    VecMove,
    /// Consuming sort; every child is authenticated before any row moves.
    VecSort,
    /// Stored row to a readonly view through the held header.
    RowsBorrow,
    /// Header length only; touches no owner.
    RowsLen,
    /// Passive marker that the row Vec settles through its held header. It is
    /// carrier-model data derived from source owner events, never cleanup-plan
    /// permission: engines settle through the validated program's full
    /// canonical cleanup plan.
    PassiveVecSettlement,
    /// Stored row back to a receipt with the held domain's issuer.
    RowsPop,
    /// Owner into a stored row; the displaced child settles first.
    RowsStore,
    /// Charged deep clone of an authenticated range.
    ChargedClone,
    /// Any route handing a private carrier to a public ABI, provider or
    /// unrelated function. Always denied.
    PublicCrossing,
    /// A stored owner copied out of its held header as a value. Always denied.
    StoredEscape,
}

/// One proved transition.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Transition {
    pub(crate) input: Carrier,
    pub(crate) output: Carrier,
    pub(crate) context: Context,
    /// Whether the slot's nonce and 32-bit generation are renewed.
    pub(crate) renews_slot: bool,
    /// Whether the header's 64-bit Vec generation advances.
    pub(crate) renews_vec: bool,
}

const fn step(
    input: Carrier,
    output: Carrier,
    context: Context,
    renews_slot: bool,
    renews_vec: bool,
) -> Option<Transition> {
    Some(Transition {
        input,
        output,
        context,
        renews_slot,
        renews_vec,
    })
}

/// The proved transition, or `None` when the profile cannot retain context.
pub(crate) const fn transition(operation: Operation) -> Option<Transition> {
    use Carrier as C;
    use Context as X;
    match operation {
        Operation::Push => step(C::Receipt, C::Stored, X::HeldRowsHeader, true, true),
        Operation::VecMove => step(C::Stored, C::Stored, X::HeldRowsHeader, false, true),
        Operation::VecSort => step(C::Stored, C::Stored, X::HeldRowsHeader, false, true),
        Operation::RowsBorrow => step(C::Stored, C::View, X::CarriedIssuer, false, false),
        Operation::RowsLen => step(C::Stored, C::Stored, X::HeldRowsHeader, false, false),
        Operation::PassiveVecSettlement => step(C::Stored, C::Stored, X::Settled, false, false),
        Operation::RowsPop => step(C::Stored, C::Receipt, X::CarriedIssuer, true, true),
        Operation::RowsStore => step(C::Owner, C::Stored, X::HeldRowsHeader, true, true),
        Operation::ChargedClone => step(C::Owner, C::OrdinaryCopy, X::Ordinary, false, false),
        Operation::PublicCrossing | Operation::StoredEscape => None,
    }
}

/// Per-slot generation renewals contributed by one operation applied to one row.
pub(crate) const fn slot_renewals(operation: Operation) -> u32 {
    match transition(operation) {
        Some(Transition {
            renews_slot: true, ..
        }) => 1,
        _ => 0,
    }
}
