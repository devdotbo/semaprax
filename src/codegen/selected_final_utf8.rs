//! Explicit native-only final-UTF8 allocation profile. The sidecar is rederived
//! from ordinary validated HIR, never trusted as supplied allocation authority.
use crate::{
    diagnostic::Diagnostic,
    hir::{DeclarationId, ResolvedProgram},
};
mod adapter;
mod certificate;
mod runtime;
#[cfg(test)]
mod tests;

pub const PROFILE: &str = "semaprax.native-final-utf8-allocation.v1";
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NativeFinalUtf8Artifact {
    source: String,
    certificate: Vec<u8>,
}
impl NativeFinalUtf8Artifact {
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn certificate(&self) -> &[u8] {
        &self.certificate
    }
}
/// Explicit selection. No default emitter, manifest, or callee name selects it.
/// The generated adapter publishes only an opaque owner handle; its caller must
/// retain the allocator state and module until the matching lease closes.
pub fn emit_native_final_utf8(
    program: &ResolvedProgram,
    entry: &DeclarationId,
    factory: &DeclarationId,
    disposer: &DeclarationId,
) -> Result<NativeFinalUtf8Artifact, Diagnostic> {
    let selection = certificate::derive(program, entry, factory, disposer)?;
    let source = super::native_emit::emit_selected_final_utf8(program, &selection)?;
    Ok(NativeFinalUtf8Artifact {
        source,
        certificate: selection.bytes,
    })
}
/// Supplied bytes are comparison input only. Replay reconstructs every grant,
/// closure identity, result layout and ordinary failure/ownership proof first.
pub fn replay_native_final_utf8(
    program: &ResolvedProgram,
    entry: &DeclarationId,
    factory: &DeclarationId,
    disposer: &DeclarationId,
    supplied: &[u8],
) -> Result<NativeFinalUtf8Artifact, Diagnostic> {
    let selection = certificate::derive(program, entry, factory, disposer)?;
    if supplied.len() > 65_536 || supplied != selection.bytes {
        return Err(error("stale or forged selected allocation certificate"));
    }
    let source = super::native_emit::emit_selected_final_utf8(program, &selection)?;
    Ok(NativeFinalUtf8Artifact {
        source,
        certificate: selection.bytes,
    })
}
pub(super) use adapter::emit_adapter;
pub(super) use certificate::Selection;
pub(super) use runtime::{emit_status_runtime, STRING_RUNTIME};
fn error(message: &str) -> Diagnostic {
    Diagnostic::io(
        "SPX-B104",
        format!("selected final-UTF8 profile: {message}"),
    )
}
