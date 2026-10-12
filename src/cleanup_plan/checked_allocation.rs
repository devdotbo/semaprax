//! Exact-source preparation for the checked allocation projection.
//!
//! This is a compiler-proof boundary, not runtime allocation permission. The
//! ordinary source/HIR/loan/inventory/plan validators run before publication.
//! Engines still require the independently replayed allocation projection and
//! their own allocator/lease receiver; no default or public route calls this
//! factory. In particular, an ordinary PropagatedCall cannot grant an allocator.
#![cfg_attr(not(test), allow(dead_code, reason = "checked allocation executors await their joined plan and lease gates"))]

use crate::{ast::Program, diagnostic::Diagnostic, hir};
use hir::{DeclarationId, FunctionExecutionId, ResolvedFunction, ResolvedProgram};

#[cfg(test)]
mod tests;

/// Original immutable source and its newly resolved, completely validated HIR.
/// No constructor accepts HIR, plans, a graph/cache receipt or an allocator.
/// The HIR has no mutation getter and retains its ordinary canonical plans.
pub(crate) struct Prepared<'source> {
    source: &'source Program,
    program: ResolvedProgram,
    entry: DeclarationId,
}

#[derive(Debug)]
pub(crate) enum Refusal {
    Source(Vec<Diagnostic>),
    Validation(Diagnostic),
    Boundary(Diagnostic),
}

/// An exact current function lent by the source owner. Its private fields and
/// callback lifetime prevent a foreign equal-content function from acquiring
/// checked projection authority. This is not an allocator or a plan receipt.
#[derive(Clone, Copy)]
pub(crate) struct Function<'hir> {
    program: &'hir ResolvedProgram,
    function: &'hir ResolvedFunction,
}
impl<'hir> Function<'hir> {
    pub(crate) fn program(self) -> &'hir ResolvedProgram { self.program }
    pub(crate) fn function(self) -> &'hir ResolvedFunction { self.function }
    pub(crate) fn held_by(self, program: &ResolvedProgram, function: &ResolvedFunction) -> bool {
        std::ptr::eq(self.program, program) && std::ptr::eq(self.function, function)
    }
}

/// First boundary: a nonescaping scalar command, with internal owning helpers.
/// Escaped String/Bytes SDK results require their own retained allocator lease
/// and are deliberately not authorized by this boundary. No ordinary signature
/// or source acceptance is changed by this explicit preparation.
pub(crate) fn prepare<'source>(
    source: &'source Program,
    entry: &DeclarationId,
) -> Result<Prepared<'source>, Refusal> {
    let program = hir::resolve(source).map_err(Refusal::Source)?;
    // Keep the complete independent check, including attached canonical plans
    // and final loan replay. A successfully resolved body is not a substitute.
    hir::validate(&program).map_err(Refusal::Validation)?;
    let function = program.functions.iter().find(|function| function.id == *entry)
        .ok_or_else(|| boundary("entry is not a current monomorphic function"))?;
    if !function.params.is_empty() || !function.effects.is_empty()
        || !matches!(function.return_type, hir::ResolvedType::I64 | hir::ResolvedType::I32
            | hir::ResolvedType::U8 | hir::ResolvedType::Usize | hir::ResolvedType::Char
            | hir::ResolvedType::F32 | hir::ResolvedType::F64 | hir::ResolvedType::Bool)
        || function.body.ownership != hir::OwnershipMode::Value
    {
        return Err(boundary("entry requires a parameter-free effect-free Copy scalar command"));
    }
    Ok(Prepared { source, program, entry: entry.clone() })
}

impl<'source> Prepared<'source> {
    pub(crate) fn source(&self) -> &'source Program { self.source }
    pub(crate) fn ordinary_program(&self) -> &ResolvedProgram { &self.program }
    pub(crate) fn entry(&self) -> &DeclarationId { &self.entry }

    /// Current membership is checked on every lending operation. No declaration
    /// name, copied function or attached evidence supplies that membership.
    pub(crate) fn with_function<R>(
        &self,
        execution: &FunctionExecutionId,
        receive: impl FnOnce(Function<'_>) -> R,
    ) -> Result<R, Refusal> {
        let function = match execution {
            FunctionExecutionId::Monomorphic(id) => self.program.functions.iter()
                .find(|function| function.id == *id),
            FunctionExecutionId::Generic(id) => self.program.function_instances.iter()
                .find(|instance| instance.id == *id).map(|instance| &instance.function),
        }.ok_or_else(|| boundary("execution is not in the exact current source closure"))?;
        Ok(receive(Function { program: &self.program, function }))
    }
}
fn boundary(message: &str) -> Refusal {
    Refusal::Boundary(Diagnostic::io("SPX-B104", format!("checked allocation preparation: {message}")))
}
