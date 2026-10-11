//! Selected ordinary shared-owner Bytes profile for OWNED-BYTE-PARTITION-V1.
//!
//! The runtime replaces the ordinary two-word `{ptr, len}` owner, whose drop is
//! `free(ptr)`, with an explicit versioned internal carrier
//! `{issuer, slot, nonce}` resolved through an explicitly held per-invocation
//! registry. No interior pointer reaches any free, no payload is preceded by a
//! probed header, and no global names the current registry. The frozen public
//! two-word ABI is untouched: public results leave only through copy-out into
//! a fresh provider allocation.
//!
//! Admission is CLOSED. [`closure::select`] reports the first reachable
//! consumer outside the closure and otherwise refuses with the cross-backend
//! join obligation; no variant of [`closure::Admission`] opens it. The private
//! fused-row lease proof grants nothing here. Nothing in the default emitter
//! reaches this module until root integration wires a selected route.
#![allow(
    dead_code,
    reason = "admission-closed selected profile; root integration wires its callers"
)]

// Loaded through `native_byte_data`, so children name their files explicitly.
#[path = "native_shared_bytes/bridge.rs"]
mod bridge;
#[path = "native_shared_bytes/closure.rs"]
pub(in crate::codegen) mod closure;
#[cfg(test)]
#[path = "native_shared_bytes/tests.rs"]
mod tests;

pub(in crate::codegen) use bridge::{emit_selected_runtime, extend_context};

/// The complete selected runtime. Its hooks default to the ordinary
/// invariant-failure boundary and the C allocator.
pub(in crate::codegen) const RUNTIME: &str = include_str!("native_shared_bytes/runtime.c");

fn error(message: impl std::fmt::Display) -> crate::diagnostic::Diagnostic {
    crate::diagnostic::Diagnostic::io(
        "SPX-B104",
        format!("selected shared-owner Bytes profile: {message}"),
    )
}
