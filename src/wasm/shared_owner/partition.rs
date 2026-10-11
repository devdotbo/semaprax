//! Generated-module receipt acquired only from the original checked source owner.
use super::*;

/// No constructor accepts caller module bytes, mutable HIR, or a closure receipt.
pub(in crate::wasm) struct PartitionModule {
    closure: Closure,
}
impl PartitionModule {
    pub(in crate::wasm) fn emit(
        owner: &crate::hir::byte_partition::ValidatedPartitionProgram<'_>,
        receiving_work: &mut usize,
    ) -> Result<Self, crate::diagnostic::Diagnostic> {
        let module = super::super::aggregate::emit_partition(owner, receiving_work)?;
        let closure = closure_impl(&module, Host::PrivateOrdinary, true)
            .map_err(|refusal| crate::diagnostic::Diagnostic::io("SPX-W110", format!("private partition consumer closure: {refusal:?}")))?;
        Ok(Self { closure })
    }
    pub(in crate::wasm) fn bytes(&self) -> &[u8] { &self.closure.module }
}

#[cfg(test)]
mod tests;
