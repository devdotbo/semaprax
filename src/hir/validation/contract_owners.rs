//! Independent closed fresh contract-local owner replay; no attached fact.
use super::super::{
    Availability, OwnershipMode, ResolvedExpr, ResolvedExprKind, ResolvedType, ValidationBinding,
    ValueId,
};
use std::collections::BTreeMap;

pub(super) fn local(expr: &ResolvedExpr, scope: &BTreeMap<ValueId, ValidationBinding>) -> bool {
    expr.ownership == OwnershipMode::Own
        && matches!(expr.ty, ResolvedType::Bytes | ResolvedType::String)
        && matches!(&expr.kind, ResolvedExprKind::Place(place) if place.projections.is_empty()
            && scope.get(&place.root).is_some_and(|binding| binding.contract_local && binding.ownership == OwnershipMode::Own
                && binding.ty == expr.ty && binding.availability == Availability::Available))
}
pub(super) fn fresh(expr: &ResolvedExpr, scope: &BTreeMap<ValueId, ValidationBinding>) -> bool {
    if expr.ownership != OwnershipMode::Own {
        return false;
    }
    if local(expr, scope) {
        return true;
    }
    match (&expr.kind, &expr.ty) {
        (ResolvedExprKind::String(_), ResolvedType::String) => true,
        (
            ResolvedExprKind::Call {
                callee,
                type_arguments,
                instance,
                args,
            },
            ResolvedType::Bytes,
        ) if crate::byte_ops::by_id(callee.as_str()) == Some(crate::byte_ops::ByteOp::Zeroed)
            && type_arguments.is_empty()
            && instance.is_none() =>
        {
            matches!(args.as_slice(), [ResolvedExpr {kind: ResolvedExprKind::Usize(n), ..}]
                    if *n <= crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES)
        }
        _ => false,
    }
}

impl super::HirValidator<'_> {
    pub(super) fn validate_contracts(
        &mut self,
        execution: &super::FunctionExecutionId,
        contracts: &[ResolvedExpr],
        outer: &BTreeMap<ValueId, ValidationBinding>,
        phase: &str,
    ) -> Result<(), crate::diagnostic::Diagnostic> {
        for (index, contract) in contracts.iter().enumerate() {
            let mut scope = outer.clone();
            // Every clause starts a new freshness domain. Entry parameters,
            // body owners and the provisional result are protected even if
            // an internal caller retained an earlier temporary scope.
            for binding in scope.values_mut() {
                binding.contract_local = false;
            }
            self.validate_expr(
                execution,
                contract,
                &mut scope,
                &format!("{phase}.{index}"),
                false,
                None,
            )?;
            self.require_type(
                &contract.ty,
                &ResolvedType::Bool,
                if phase == "requires" {
                    "precondition"
                } else {
                    "postcondition"
                },
            )?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests;
