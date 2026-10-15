//! Exact admission receives paid scratch from the same live account. Ordinary
//! classifiers remain independent owners; no attached verdict is consulted.
use super::*;
use crate::hir::{DeclarationIndex, DeclarationKind, IdentityOrigin, ResolvedFieldDeclaration};
mod byte;
mod collection;
mod text;
const MAX_NESTED_OWNED_RECORD_DEPTH: usize = 64;
const MAX_NESTED_OWNED_BYTE_LEAVES: usize = 256;
const MAX_NESTED_OWNED_RECORD_FIELDS: usize = 4_096;
fn nested_record_copy_scalar_is_admitted(ty: &ResolvedType) -> bool {
    matches!(
        ty,
        ResolvedType::I64
            | ResolvedType::I32
            | ResolvedType::Char
            | ResolvedType::U8
            | ResolvedType::Usize
            | ResolvedType::F32
            | ResolvedType::F64
            | ResolvedType::Bool
    )
}
fn monomorphic_record(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    matches!(ty, ResolvedType::Nominal { declaration, arguments }
        if arguments.is_empty() && index.type_parameters(declaration).is_some_and(|p| p.is_empty())
        && index.declaration(declaration).is_some_and(|d| d.kind == DeclarationKind::Record && d.identity_origin == IdentityOrigin::Explicit))
}
fn byte(
    index: &DeclarationIndex,
    ty: &ResolvedType,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    match ty {
        ResolvedType::Bytes => Ok(true),
        ResolvedType::Nominal {
            declaration,
            arguments,
        } => byte::byte_head(index, declaration, arguments, function, budget),
        _ => Ok(false),
    }
}
fn runtime_text(
    index: &DeclarationIndex,
    ty: &ResolvedType,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    if text::text(ty, index, function, budget)? {
        Ok(true)
    } else {
        collection::collection(ty, index, function, budget)
    }
}
pub(super) fn subtree(
    index: &DeclarationIndex,
    root: &ResolvedType,
    child: &ResolvedType,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    budget.charge(function, 1, "whole subtree admission")?;
    if !monomorphic_record(index, root) || !monomorphic_record(index, child) {
        return Ok(false);
    }
    if !byte(index, root, function, budget)? && !runtime_text(index, root, function, budget)? {
        return Ok(false);
    }
    Ok(byte(index, child, function, budget)? || runtime_text(index, child, function, budget)?)
}
/// Reachable vector helpers are borrowed field scans with fixed local arrays:
/// vec_ops scalar, copy_record fields, owned_leaf layout and three-field owned
/// record admission allocate no heap. Preserve the complete original receiver.
pub(super) fn vector(index: &DeclarationIndex, ty: &ResolvedType) -> bool {
    crate::hir::owned_collection_record::vector(index, ty)
}

#[cfg(test)]
mod tests;
