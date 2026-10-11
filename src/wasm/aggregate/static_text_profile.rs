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

/// Full compiler proof is separate from the metered current-graph receipt.
pub(in crate::wasm) fn emit_partition(
    owner: &crate::hir::byte_partition::ValidatedPartitionProgram<'_>,
    receiving_work: &mut usize,
) -> Result<Vec<u8>, Diagnostic> {
    owner.revalidate_compiler()?;
    owner.receive_current_graph(receiving_work)?;
    emit_profile_with_scalar_exports(owner.program(), false, false, &[], Some(false))
}
