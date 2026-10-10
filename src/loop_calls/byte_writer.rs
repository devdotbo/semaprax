//! Bounded transitive proof for ordinary same-owner byte-writing helpers.
//! Declaration spelling, attached cleanup and generated names carry no authority.
use crate::ast::{Expr, ExprKind, Function, ParamMode, Program, Type};
use crate::hir::{
    OwnershipMode, ResolvedBinding, ResolvedExpr, ResolvedExprKind, ResolvedFunction,
    ResolvedProgram, ResolvedType,
};
mod resolved;
mod source;
pub(crate) use resolved::{
    bindings, checked_bindings, nonallocating as resolved_nonallocating,
    nonallocating_with as resolved_nonallocating_with,
};
pub(crate) use source::nonallocating as source_nonallocating;
const DEPTH: usize = crate::cache_codec::MAX_DEPTH;
const WORK: usize = crate::cache_codec::MAX_NODES;

fn byte_operation(op: crate::byte_ops::ByteOp) -> bool {
    matches!(
        op,
        crate::byte_ops::ByteOp::Len
            | crate::byte_ops::ByteOp::Get
            | crate::byte_ops::ByteOp::Range
            | crate::byte_ops::ByteOp::BytesAsSlice
            | crate::byte_ops::ByteOp::ArrayAsSlice
            | crate::byte_ops::ByteOp::StrAsBytes
            | crate::byte_ops::ByteOp::StringAsStr
            | crate::byte_ops::ByteOp::Set
            | crate::byte_ops::ByteOp::Set5
            | crate::byte_ops::ByteOp::Set1Or5
            | crate::byte_ops::ByteOp::Set1Or6Or48
    )
}
fn text_operation(op: crate::string_ops::StringOp) -> bool {
    use crate::string_ops::StringOp::*;
    matches!(
        op,
        Len | IsEmpty | StartsWith | Contains | LenChars | Find | ToI64 | ByteAt | Compare
    ) || (crate::hir::is_scalar_resolved_type(&op.return_type())
        && !matches!(op, MapGetOr | MapHas | MapLen | MapValueAt))
}
fn vec_operation(op: crate::vec_ops::VecOp) -> bool {
    matches!(
        op,
        crate::vec_ops::VecOp::Len | crate::vec_ops::VecOp::Capacity | crate::vec_ops::VecOp::Get
    )
}
fn ast_param(program: &Program, mode: ParamMode, ty: &Type) -> bool {
    match mode {
        ParamMode::Own => *ty == Type::Bytes,
        ParamMode::Value => {
            crate::source_verify::is_scalar_source_type(ty)
                || crate::source_verify::copy_record_source_admitted(program, ty)
                || super::ast_copy_variant(program, ty)
        }
        ParamMode::Borrow => {
            super::ast_param_admitted(program, mode, ty)
                || matches!(ty, Type::Named { name, arguments } if arguments.is_empty()
                && program.types.iter().any(|d| d.name == *name && matches!(d.kind, crate::ast::TypeDeclarationKind::Record { .. })))
        }
        ParamMode::Shared => false,
    }
}
pub(crate) fn source_target(program: &Program, target: &Function) -> bool {
    target.return_type == Type::Bytes
        && target
            .params
            .iter()
            .filter(|p| p.mode == ParamMode::Own && p.ty == Type::Bytes)
            .count()
            == 1
        && target
            .params
            .iter()
            .all(|p| ast_param(program, p.mode, &p.ty))
        && source_nonallocating(program, target)
}
pub(crate) fn source_assignment(program: &Program, binding: &str, value: &Expr) -> bool {
    let ExprKind::Call {
        name,
        type_arguments,
        args,
    } = &value.kind
    else {
        return false;
    };
    let Some(target) = program.functions.iter().find(|f| f.name == *name) else {
        return false;
    };
    type_arguments.is_empty()
        && args.len() == target.params.len()
        && source_target(program, target)
        && target.params.iter().zip(args).all(|(p, arg)| {
            p.mode != ParamMode::Own || matches!(&arg.kind, ExprKind::Var(name) if name == binding)
        })
}
fn resolved_param(program: &ResolvedProgram, mode: OwnershipMode, ty: &ResolvedType) -> bool {
    match mode {
        OwnershipMode::Own => *ty == ResolvedType::Bytes,
        OwnershipMode::Value => {
            crate::hir::is_scalar_resolved_type(ty)
                || crate::hir::copy_record_collection::admitted(&program.declarations, ty)
                || super::resolved_match_scrutinee_admitted(&program.declarations, ty)
        }
        OwnershipMode::Borrow => {
            super::resolved_param_admitted(&program.declarations, mode, ty)
                || matches!(ty, ResolvedType::Nominal { declaration, arguments } if arguments.is_empty() && program.declarations.record_fields(declaration).is_some())
                || crate::hir::owned_collection_record::admitted(ty, &program.declarations)
        }
        OwnershipMode::Shared => false,
    }
}
pub(crate) fn resolved_target(program: &ResolvedProgram, target: &ResolvedFunction) -> bool {
    target.return_type == ResolvedType::Bytes
        && target
            .params
            .iter()
            .filter(|p| p.ownership == OwnershipMode::Own && p.ty == ResolvedType::Bytes)
            .count()
            == 1
        && target
            .params
            .iter()
            .all(|p| resolved_param(program, p.ownership, &p.ty))
        && resolved_nonallocating(program, target)
}
pub(crate) fn assignment(
    program: &ResolvedProgram,
    binding: &ResolvedBinding,
    value: &ResolvedExpr,
) -> bool {
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &value.kind
    else {
        return false;
    };
    let Some(target) = program.resolve_call_target(callee, None) else {
        return false;
    };
    binding.ty == ResolvedType::Bytes && binding.ownership == OwnershipMode::Own
        && value.ty == ResolvedType::Bytes && value.ownership == OwnershipMode::Own
        && type_arguments.is_empty() && args.len() == target.params.len()
        && resolved_target(program, target)
        && target.params.iter().zip(args).all(|(p, arg)| {
            p.ty == arg.ty && match p.ownership {
                OwnershipMode::Own => arg.ownership == OwnershipMode::Own
                    && matches!(&arg.kind, ResolvedExprKind::Place(place) if place.root == binding.id && place.projections.is_empty()),
                OwnershipMode::Value => arg.ownership == OwnershipMode::Value,
                OwnershipMode::Borrow => matches!(arg.ownership, OwnershipMode::Own | OwnershipMode::Borrow),
                OwnershipMode::Shared => false,
            }
        })
}

#[cfg(test)]
mod tests;
