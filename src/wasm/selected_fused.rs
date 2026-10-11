//! Selected `FusedConstructionV1` Core-Wasm executor for OPT #747.
//!
//! Default admission is closed: no public emitter, profile or route calls this
//! module. The only publishing entry, [`accept`], receives the private
//! `hir::selected_fused_candidate::ValidatedFusedSelection` owner, whose only
//! constructor (`validate_selected`) has run the source passes, resolution of
//! the original source, the independent typed join, full selected validation
//! and selected capacity. `accept` rederives every node it lowers inside the
//! owner's fresh `with_joined` callback, requires `Joined::still_current`,
//! cross-checks the owner's capacity facts and then runs every executor-side
//! check below. Raw HIR or a bare `Joined` is never accepted. The older
//! descriptor, capacity summary and rescaled 32-row HIR remain an unvalidated
//! test-only precursor (`precursor`); they produce no [`Artifact`] and grant
//! no count authority.
//!
//! Lowering consumes the function's exact canonical cleanup plan: admission
//! revalidates the attached cleanup and loan plans against fresh builds, the
//! emitter replays every transition into Wasm liveness flags, and every exit
//! emits its `finalize_in_order` vector verbatim. A failure selects its status
//! with the host before any cleanup, so cleanup cannot replace it.
//!
//! The module imports one private namespace, uses only Wasm 1.0 value types
//! and is never mixed with a public `env` profile. Every handle is minted by
//! the realm's single caller-held never-reset namespace (`host.mjs`), which
//! binds host, instance, invocation, owner domain and kind to each token.
#![cfg_attr(
    not(test),
    allow(
        dead_code,
        reason = "reached only after root source, typed-HIR and selected validation activate"
    )
)]
use wasm_encoder::ValType;

mod cleanup;
mod facts;
mod layout;
mod lower;
#[cfg(test)]
mod precursor;
mod suffix;
#[cfg(test)]
mod tests;

use facts::Selected;

/// Private host namespace. It is not a frozen public Bytes ABI.
pub(in crate::wasm) const NAMESPACE: &str = "semaprax.private.selected-fused.v1";
/// Only export besides the module's private literal memory.
pub(in crate::wasm) const ENTRY: &str = "semaprax_main";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(in crate::wasm) enum Refusal {
    /// No closed current source/HIR construction (an ordinary program).
    Join,
    /// The producer-bound carrier mapping refused.
    Carrier,
    /// The capacity summary was not analyzed under `FusedConstructionV1`.
    Strategy,
    /// No single fused backing for the selected function, or an ordinary
    /// limit is exceeded.
    Capacity,
    /// At most 32 rows: the ordinary finite profile owns the program.
    Legacy,
    /// Rows, width, extent or the actual row Vec layout do not fit.
    Layout,
    /// The attached cleanup plan is not the exact canonical plan, or the
    /// lowering did not reproduce every transition and exit.
    Cleanup,
    /// The attached loan plan is not the exact canonical plan, or a leased
    /// view outlives its authenticated loan scope.
    Loans,
    /// The input no longer borrows the exact source and HIR.
    Stale,
    /// Lowering reached a construct outside the closed use domain.
    Unsupported(&'static str),
}

type Signature = (&'static str, &'static [ValType], &'static [ValType]);
const L: ValType = ValType::I64;
const W: ValType = ValType::I32;

/// Exact import inventory, in index order. `host.mjs` mirrors these names and
/// refuses a module whose import list differs in any position.
pub(in crate::wasm) const IMPORTS: [Signature; 24] = [
    ("start", &[L, L], &[L]),
    ("vec_new", &[L, L], &[L]),
    ("string_literal", &[L, W, W], &[L]),
    ("issue", &[L, L], &[L]),
    ("row_push", &[L, L, L, L, L], &[L]),
    ("complete", &[L], &[W]),
    ("abort", &[L], &[W]),
    ("settle_abort", &[L], &[W]),
    ("vec_move", &[L, L], &[L]),
    ("vec_len", &[L, L], &[L]),
    ("field_scalar", &[L, L, L], &[L]),
    ("field_text", &[L, L, L], &[L]),
    ("text_len", &[L, L], &[L]),
    ("text_byte", &[L, L, L], &[W]),
    ("payload_borrow", &[L, L, L], &[L]),
    ("view_len", &[L, L], &[L]),
    ("view_byte", &[L, L, L], &[W]),
    ("end_borrow", &[L, L], &[W]),
    ("sort", &[L, L], &[L]),
    ("vec_drop", &[L, L], &[W]),
    ("string_drop", &[L, L], &[W]),
    ("owner_drop", &[L, L], &[W]),
    ("select_failure", &[W], &[]),
    ("return_failure", &[], &[]),
];

/// Import function indexes; each equals its position in [`IMPORTS`].
pub(super) mod import {
    pub(crate) const START: u32 = 0;
    pub(crate) const VEC_NEW: u32 = 1;
    pub(crate) const STRING_LITERAL: u32 = 2;
    pub(crate) const ISSUE: u32 = 3;
    pub(crate) const ROW_PUSH: u32 = 4;
    pub(crate) const COMPLETE: u32 = 5;
    pub(crate) const ABORT: u32 = 6;
    pub(crate) const SETTLE_ABORT: u32 = 7;
    pub(crate) const VEC_MOVE: u32 = 8;
    pub(crate) const VEC_LEN: u32 = 9;
    pub(crate) const FIELD_SCALAR: u32 = 10;
    pub(crate) const FIELD_TEXT: u32 = 11;
    pub(crate) const TEXT_LEN: u32 = 12;
    pub(crate) const TEXT_BYTE: u32 = 13;
    pub(crate) const PAYLOAD_BORROW: u32 = 14;
    pub(crate) const VIEW_LEN: u32 = 15;
    pub(crate) const VIEW_BYTE: u32 = 16;
    pub(crate) const END_BORROW: u32 = 17;
    pub(crate) const SORT: u32 = 18;
    pub(crate) const VEC_DROP: u32 = 19;
    pub(crate) const STRING_DROP: u32 = 20;
    pub(crate) const OWNER_DROP: u32 = 21;
    pub(crate) const SELECT_FAILURE: u32 = 22;
    pub(crate) const RETURN_FAILURE: u32 = 23;
}

/// Host grant for exactly one emitted module. The host recomputes every
/// dimension from its own physical layout and refuses any disagreement; the
/// grant is a cross-check bound to the module digest, not an authority token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(in crate::wasm) struct Grant {
    pub(in crate::wasm) digest: String,
    pub(in crate::wasm) rows: u32,
    pub(in crate::wasm) width: u64,
    /// Lease and view tables carved from the invocation account.
    pub(in crate::wasm) metadata_bytes: u64,
    pub(in crate::wasm) row_bytes: u64,
    /// Complete per-invocation physical account: the precharged registry
    /// entries' bytes, then row backing, then lease/view tables.
    pub(in crate::wasm) account_bytes: u64,
    /// Namespace registry entries the host precharges at `start`.
    pub(in crate::wasm) registry_entries: u64,
    /// Field roles in declaration order, as `scalar`, `text` and `payload`.
    pub(in crate::wasm) order: [&'static str; 3],
}

/// Publishable selected module. Only [`accept`] constructs one.
pub(in crate::wasm) struct Artifact {
    pub(in crate::wasm) bytes: Vec<u8>,
    pub(in crate::wasm) grant: Grant,
}

/// The selected publishing entry. Every lowered node is rederived inside the
/// owner's fresh join; nothing from an earlier join, the precursor or raw HIR
/// is consulted. The owner's capacity facts must agree with the joined
/// dimensions, and then every executor-side check runs before lowering.
pub(in crate::wasm) fn accept(
    owner: &crate::hir::selected_fused_candidate::ValidatedFusedSelection<'_>,
) -> Result<Artifact, Refusal> {
    let source = owner.source();
    let program = owner.program();
    let capacity = owner.capacity();
    // Refusal-only carrier cross-check; carriers themselves come from their
    // producing field reads inside the lowering.
    let mapping = crate::construction_fusion::native_provenance::derive(source, program)
        .ok_or(Refusal::Carrier)?;
    owner
        .with_joined(|joined| {
            if !joined.still_current(source, program) {
                return Err(Refusal::Stale);
            }
            let selected = facts::from_joined(joined, source, program, Some(&mapping))?;
            let extent = u64::from(selected.rows)
                .checked_mul(selected.width)
                .ok_or(Refusal::Layout)?;
            if capacity.rows != selected.rows
                || capacity.width != selected.width
                || capacity.extent != extent
                || capacity.fused_backing_events != 1
                || capacity.bytes_copy_sites == 0
                || capacity.bytes_copy_sites > crate::byte_data_capacity::MAX_BYTES_COPY_SITES
                || capacity.owned_byte_payload_bytes < extent
                || capacity.owned_byte_payload_bytes
                    > crate::byte_data_capacity::MAX_OWNED_BYTE_PAYLOAD_BYTES
            {
                return Err(Refusal::Capacity);
            }
            facts::check(&selected, source, program)?;
            let (bytes, grant) = module(&selected)?;
            Ok(Artifact { bytes, grant })
        })
        .map_err(|_| Refusal::Join)?
}

/// Lower an already checked selection. Callers must have run [`facts::check`]
/// against the exact current source and HIR.
fn module(selected: &Selected<'_>) -> Result<(Vec<u8>, Grant), Refusal> {
    let bytes = lower::module(selected)?;
    let digest = {
        use sha2::{Digest, Sha256};
        Sha256::digest(&bytes)
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect()
    };
    let grant = Grant {
        digest,
        rows: selected.rows,
        width: selected.width,
        metadata_bytes: layout::metadata_bytes(selected.rows),
        row_bytes: layout::ROW_BYTES,
        account_bytes: layout::account_bytes(selected.rows),
        registry_entries: layout::registry_entries(selected.rows),
        order: selected.order,
    };
    Ok((bytes, grant))
}
