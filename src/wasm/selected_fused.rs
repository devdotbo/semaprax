//! Selected `FusedConstructionV1` Core-Wasm executor for OPT #747.
//!
//! Default admission is closed. The only publishing entry, [`accept`], takes a
//! [`ValidatedSelection`], which cannot be constructed until root supplies the
//! independently authenticated source proof, the typed source/HIR join and
//! complete selected validation. The older descriptor, capacity summary and
//! rescaled 32-row HIR remain an unvalidated test-only precursor
//! (`precursor`); they produce no [`Artifact`] and grant no count authority.
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
    /// Complete invocation account: row backing plus lease/view tables.
    pub(in crate::wasm) account_bytes: u64,
    /// Namespace registry entries the host precharges at `start`.
    pub(in crate::wasm) registry_entries: u64,
    /// Field roles in declaration order, as `scalar`, `text` and `payload`.
    pub(in crate::wasm) order: [&'static str; 3],
}

/// Publishable selected module. Only [`accept`] constructs one.
#[allow(dead_code, reason = "constructed only after root selected activation")]
pub(in crate::wasm) struct Artifact {
    pub(in crate::wasm) bytes: Vec<u8>,
    pub(in crate::wasm) grant: Grant,
}

/// Not constructible until root activation: it will carry the authenticated
/// `SourceProof`, the `hir::fused_construction::join::Joined` selection, the
/// producer-bound carrier mapping and the complete selected validation
/// receipt. The uninhabited field keeps every publishing path closed.
#[allow(dead_code, reason = "constructed only after root selected activation")]
pub(in crate::wasm) struct ValidatedSelection<'a> {
    pending: PendingRootValidation,
    held: std::marker::PhantomData<&'a crate::hir::ResolvedProgram>,
}
enum PendingRootValidation {}

/// The selected publishing entry. Root hookup replaces the uninhabited field
/// with its validated owner, builds [`Selected`] from it, calls
/// [`facts::check`] and then [`module`]; no step may be skipped or rebuilt
/// from the unvalidated precursor.
#[allow(dead_code, reason = "called only after root selected activation")]
pub(in crate::wasm) fn accept(selection: ValidatedSelection<'_>) -> Result<Artifact, Refusal> {
    match selection.pending {}
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
