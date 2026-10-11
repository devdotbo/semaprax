//! Exact selected ordinary-loop cell facts; no allocated legacy census.
use super::*;
use crate::hir::{OwnershipMode, ResolvedExpr, ResolvedType};
pub(super) fn vector(
    program: &dyn crate::cleanup::ReplayContext,
    binding: &ResolvedBinding,
    value: &ResolvedExpr,
) -> bool {
    let Ok(Some(element)) = crate::cleanup::selected_vector_element(program, &binding.ty) else {
        return false;
    };
    let ResolvedExprKind::Call {
        callee,
        instance: None,
        type_arguments,
        args,
    } = &value.kind
    else {
        return false;
    };
    let Some(op) = crate::vec_ops::by_id(callee.as_str()).filter(|op| op.reopens_same_owner())
    else {
        return false;
    };
    if binding.ownership != OwnershipMode::Own
        || value.ownership != OwnershipMode::Own
        || value.ty != binding.ty
        || type_arguments.as_slice() != std::slice::from_ref(element)
        || args.len() != op.arity()
    {
        return false;
    }
    if !(crate::vec_ops::resolved_operation_element_is_admitted(op, element)
        || crate::hir::owned_leaf_collection::vec_operation_admitted(
            program.declarations(),
            op,
            element,
        ))
    {
        return false;
    }
    let owned = *element == ResolvedType::Bytes
        || *element == ResolvedType::String
        || crate::hir::owned_leaf_collection::runtime_element(program.declarations(), element);
    args.iter().enumerate().all(|(i,arg)|{
        let mode=if owned{op.param_ownership_for(i,element)}else{op.param_ownership(i)};
        let ty=match i {0=>arg.ty==binding.ty,1 if op==crate::vec_ops::VecOp::Push =>&arg.ty==element,2=>&arg.ty==element,_=>arg.ty==ResolvedType::Usize};
        ty&&arg.ownership==mode&&(i!=0||matches!(&arg.kind,ResolvedExprKind::Place(p) if p.root==binding.id&&p.projections.is_empty()))
    })
}
