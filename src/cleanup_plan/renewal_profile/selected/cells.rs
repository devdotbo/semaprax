//! Exact selected ordinary-loop cell facts; no allocated legacy census.
use super::*;
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedType};
pub(super) fn vector(
    program: &dyn crate::cleanup::ReplayContext,
    binding: &ResolvedBinding,
    value: &ResolvedExpr,
    charge: &mut dyn FnMut(usize) -> Result<(), crate::diagnostic::Diagnostic>,
) -> Result<bool, crate::diagnostic::Diagnostic> {
    let query = match &binding.ty {
        ResolvedType::Nominal { declaration, .. } => declaration.as_str().len(),
        _ => return Ok(false),
    };
    crate::cleanup::prepay_selected_collection_probes(program, query, charge)?;
    let Ok(Some(element)) = crate::cleanup::selected_vector_element(program, &binding.ty) else {
        return Ok(false);
    };
    let ResolvedExprKind::Call {
        callee,
        instance: None,
        type_arguments,
        args,
    } = &value.kind
    else {
        return Ok(false);
    };
    charge(
        callee
            .as_str()
            .len()
            .min(64)
            .checked_add(1)
            .and_then(|n| n.checked_mul(32))
            .ok_or_else(super::error)?,
    )?;
    charge(
        binding
            .id
            .as_str()
            .len()
            .checked_add(1)
            .and_then(|n| n.checked_mul(4))
            .ok_or_else(super::error)?,
    )?;
    let Some(op) = crate::vec_ops::by_id(callee.as_str()).filter(|op| op.reopens_same_owner())
    else {
        return Ok(false);
    };
    if binding.ownership != OwnershipMode::Own
        || value.ownership != OwnershipMode::Own
        || value.ty != binding.ty
        || type_arguments.as_slice() != std::slice::from_ref(element)
        || args.len() != op.arity()
    {
        return Ok(false);
    }
    if !(crate::vec_ops::resolved_operation_element_is_admitted(op, element)
        || crate::hir::owned_leaf_collection::vec_operation_admitted(
            program.declarations(),
            op,
            element,
        ))
    {
        return Ok(false);
    }
    let owned = *element == ResolvedType::Bytes
        || *element == ResolvedType::String
        || crate::hir::owned_leaf_collection::runtime_element(program.declarations(), element);
    Ok(args.iter().enumerate().all(|(i,arg)|{
        let mode=if owned{op.param_ownership_for(i,element)}else{op.param_ownership(i)};
        let ty=match i {0=>arg.ty==binding.ty,1 if op==crate::vec_ops::VecOp::Push =>&arg.ty==element,2=>&arg.ty==element,_=>arg.ty==ResolvedType::Usize};
        ty&&arg.ownership==mode&&(i!=0||matches!(&arg.kind,ResolvedExprKind::Place(p) if p.root==binding.id&&p.projections.is_empty()))
    }))
}
