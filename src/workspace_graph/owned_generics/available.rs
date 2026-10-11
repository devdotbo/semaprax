//! Borrowed closure discovery; chosen final linker inputs remain owned.
use crate::hir;
pub(super) struct BorrowedFunction<'a> {
    pub(super) function: &'a hir::ResolvedFunction,
    pub(super) origin: hir::IdentityOrigin,
}
pub(in crate::workspace_graph) trait FunctionView {
    fn function(&self) -> &hir::ResolvedFunction;
}
impl FunctionView for BorrowedFunction<'_> {
    fn function(&self) -> &hir::ResolvedFunction {
        self.function
    }
}
impl FunctionView for hir::LinkedScalarFunction {
    fn function(&self) -> &hir::ResolvedFunction {
        &self.function
    }
}
