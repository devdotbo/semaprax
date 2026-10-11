//! Closed non-owning signatures and rooted reads for retained helper replay.
//! This is a shape check; current defining source and cleanup proofs remain required.
use super::{
    OwnershipMode as O, ResolvedExpr, ResolvedExprKind as K, ResolvedFunction, ResolvedType as T,
};
pub(crate) fn parameter(ty: &T, mode: O) -> bool {
    (mode == O::Value && super::is_scalar_resolved_type(ty))
        || (mode == O::Borrow && matches!(ty, T::Str | T::SliceU8))
}
pub(crate) fn signature(function: &ResolvedFunction) -> bool {
    super::is_scalar_resolved_type(&function.return_type)
        && function
            .params
            .iter()
            .all(|p| parameter(&p.ty, p.ownership))
}
pub(crate) fn builtin(id: &str) -> bool {
    id == crate::byte_ops::LEN_ID
        || matches!(
            crate::str_ops::by_id(id),
            Some(
                crate::str_ops::StrOp::LenBytes
                    | crate::str_ops::StrOp::IsEmpty
                    | crate::str_ops::StrOp::StartsWith
                    | crate::str_ops::StrOp::Contains
            )
        )
}
fn root<E>(
    function: &ResolvedFunction,
    place: &super::Place,
    ty: &T,
    charge: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    if !place.projections.is_empty() {
        return Ok(false);
    }
    let mut found = false;
    for parameter in &function.params {
        // The exact current parameter identity is checked; no alias or owned root
        // receives authority from a nominally borrowed expression type.
        charge(parameter.id.as_str().len())?;
        charge(place.root.as_str().len())?;
        charge(1)?;
        if parameter.id == place.root {
            if found || parameter.ownership != O::Borrow || &parameter.ty != ty {
                return Ok(false);
            }
            found = true;
        }
    }
    Ok(found)
}
pub(crate) fn expression<E>(
    function: &ResolvedFunction,
    expr: &ResolvedExpr,
    charge: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
    let scalar = super::is_scalar_resolved_type(&expr.ty) && expr.ownership == O::Value;
    match &expr.kind {
        K::Place(place) if matches!(expr.ty, T::Str | T::SliceU8) => {
            Ok(expr.ownership == O::Borrow && root(function, place, &expr.ty, charge)?)
        }
        K::BorrowPlace { operation, place } => Ok(expr.ownership == O::Borrow
            && expr.ty == T::SliceU8
            && operation.as_str() == crate::byte_ops::STR_AS_BYTES_ID
            && root(function, place, &T::Str, charge)?),
        K::Call {
            callee,
            instance,
            type_arguments,
            args,
        } if builtin(callee.as_str()) => {
            charge(args.len())?;
            charge(1)?;
            if !scalar || instance.is_some() || !type_arguments.is_empty() {
                return Ok(false);
            }
            if callee.as_str() == crate::byte_ops::LEN_ID {
                return Ok(expr.ty == T::Usize
                    && args.len() == 1
                    && args[0].ty == T::SliceU8
                    && args[0].ownership == O::Borrow);
            }
            let Some(operation) = crate::str_ops::by_id(callee.as_str()) else {
                return Ok(false);
            };
            Ok(expr.ty == operation.return_type()
                && args.len() == operation.arity()
                && args
                    .iter()
                    .all(|arg| arg.ty == T::Str && arg.ownership == O::Borrow))
        }
        K::Call { callee, .. }
            if crate::byte_ops::by_id(callee.as_str()).is_some()
                || crate::str_ops::by_id(callee.as_str()).is_some()
                || crate::string_ops::by_id(callee.as_str()).is_some() =>
        {
            Ok(false)
        }
        _ => Ok(scalar),
    }
}
