//! Ordinary dynamic operation shape, shared by the pending source/HIR/runtime
//! receivers. This is not a validation lease or builtin-registry grant.
use super::{debit, equal, geometry, Geometry, Preparation, ID, NAME};
use crate::{ast, hir};

pub(crate) const STATUS_DOMAIN: &str = "semaprax.byte-partition.v1";
/// Codes are local to the new partition domain; existing byte domains do not change.
pub(crate) const fn fault_code(fault: super::Fault) -> u32 {
    match fault {
        super::Fault::WidthZero => 1,
        super::Fault::ExtentOverflow => 2,
        super::Fault::ExtentMismatch => 3,
        super::Fault::MetadataCapacity => 4,
    }
}
/// Shared bindings generated from the authoritative operation variants.
pub(crate) const STATUS_CODES: [u32; 4] = [
    fault_code(super::Fault::WidthZero),
    fault_code(super::Fault::ExtentOverflow),
    fault_code(super::Fault::ExtentMismatch),
    fault_code(super::Fault::MetadataCapacity),
];
pub(crate) struct Operands<'a> {
    pub(crate) owner: &'a hir::ResolvedExpr,
    pub(crate) count: &'a hir::ResolvedExpr,
    pub(crate) width: &'a hir::ResolvedExpr,
}
/// Borrow the actual operands, never evaluated values or reconstructed nodes.
/// The runtime must additionally authenticate WHOLE origin before consuming;
/// a child has the same language Bytes type and must not pass that receiver.
pub(crate) fn typed<'a>(
    expression: &'a hir::ResolvedExpr,
    work: &mut usize,
) -> Result<Option<Operands<'a>>, super::Refusal> {
    let result = (|| {
        debit(work, 1)?;
        let hir::ResolvedExprKind::Call {
            callee,
            instance,
            type_arguments,
            args,
        } = &expression.kind
        else {
            return Ok(None);
        };
        if !equal(callee.as_str(), ID, work)? {
            return Ok(None);
        }
        debit(work, 8)?;
        let [owner, count, width] = args.as_slice() else {
            return Err(super::Refusal::Identity);
        };
        let hir::ResolvedType::Nominal {
            declaration,
            arguments,
        } = &expression.ty
        else {
            return Err(super::Refusal::Identity);
        };
        if instance.is_some()
            || !type_arguments.is_empty()
            || expression.ownership != hir::OwnershipMode::Own
            || !equal(declaration.as_str(), crate::prelude::VEC_ID, work)?
            || arguments.as_slice() != [hir::ResolvedType::Bytes]
            || owner.ty != hir::ResolvedType::Bytes
            || owner.ownership != hir::OwnershipMode::Own
            || !matches!(&owner.kind, hir::ResolvedExprKind::Place(place) if place.projections.is_empty())
            || count.ty != hir::ResolvedType::Usize
            || count.ownership != hir::OwnershipMode::Value
            || width.ty != hir::ResolvedType::Usize
            || width.ownership != hir::OwnershipMode::Value
        {
            return Err(super::Refusal::Identity);
        }
        Ok(Some(Operands {
            owner,
            count,
            width,
        }))
    })();
    if result.is_err() {
        *work = 0;
    }
    result
}
/// Source spelling is only a candidate. Ordinary name resolution and the
/// independent current-HIR receiver must resolve ID before execution.
pub(crate) fn source<'a>(
    expression: &'a ast::Expr,
    work: &mut usize,
) -> Result<Option<&'a [ast::Expr]>, super::Refusal> {
    let result = (|| {
        debit(work, 1)?;
        let ast::ExprKind::Call {
            name,
            type_arguments,
            args,
        } = &expression.kind
        else {
            return Ok(None);
        };
        if !equal(name, NAME, work)? {
            return Ok(None);
        }
        debit(work, 4)?;
        if !type_arguments.is_empty()
            || args.len() != 3
            || !matches!(args[0].kind, ast::ExprKind::Var(_))
        {
            return Err(super::Refusal::Identity);
        }
        Ok(Some(args.as_slice()))
    })();
    if result.is_err() {
        *work = 0;
    }
    result
}
/// Dynamic u64 operand values retain semantic-usize behavior on all targets.
/// Metadata allocation is deliberately after this result and uses actual
/// executor layouts, not a guessed count multiplier or a new payload budget.
pub(crate) fn prepare(
    authenticated_whole_extent: u64,
    evaluated_count: u64,
    evaluated_width: u64,
) -> Result<Geometry, Preparation> {
    geometry(authenticated_whole_extent, evaluated_count, evaluated_width)
}

#[cfg(test)]
mod tests;
