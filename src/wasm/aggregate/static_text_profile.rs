//! Invocation-local selection returned by complete mandatory HIR validation.
//! It is not retained metadata or public profile authority.
use super::*;

pub(in crate::wasm) fn emit_with_static_text(
    program: &ResolvedProgram,
    selected: bool,
) -> Result<Vec<u8>, Diagnostic> {
    emit_profile_with_scalar_exports(program, false, false, &[], Some(selected))
}

pub(in crate::wasm) fn emit_scalar_exports_with_static_text(
    program: &ResolvedProgram,
    plans: &[super::super::scalar_exports::ScalarExportPlan],
    selected: bool,
) -> Result<Vec<u8>, Diagnostic> {
    emit_profile_with_scalar_exports(program, false, false, plans, Some(selected))
}
