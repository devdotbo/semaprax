//! Actual comparison operands, never a multiplier over an authored body.
use super::{Admission, Refusal, Scratch};
use crate::hir::{ResolvedType, ValueId};
pub(super) fn type_tree(ty: &ResolvedType, admission: &mut Admission<'_>) -> Result<(), Refusal> {
    admission.units(2)?; // census and unchanged equality traversal
    match ty {
        ResolvedType::Nominal {
            declaration,
            arguments,
        } if arguments.is_empty() => return admission.units(declaration.as_str().len()),
        ResolvedType::TypeParameter { owner, .. } => return admission.units(owner.as_str().len()),
        ResolvedType::Nominal { .. } | ResolvedType::Function { .. } => {}
        _ => return Ok(()),
    }
    let mut pending = Scratch::new(admission)?;
    pending.push(ty, admission)?;
    while let Some(ty) = pending.pop(admission)? {
        match ty {
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                admission.units(declaration.as_str().len())?;
                for argument in arguments {
                    admission.units(2)?;
                    pending.push(argument, admission)?;
                }
            }
            ResolvedType::TypeParameter { owner, .. } => admission.units(owner.as_str().len())?,
            ResolvedType::Function { parameters, result } => {
                for parameter in parameters {
                    admission.units(2)?;
                    pending.push(parameter, admission)?;
                }
                admission.units(2)?;
                pending.push(result.as_ref(), admission)?;
            }
            _ => {}
        }
    }
    Ok(())
}
pub(super) fn types(
    left: &ResolvedType,
    right: &ResolvedType,
    admission: &mut Admission<'_>,
) -> Result<bool, Refusal> {
    type_tree(left, admission)?;
    type_tree(right, admission)?;
    Ok(left == right)
}
pub(super) fn text(
    left: &str,
    right: &str,
    admission: &mut Admission<'_>,
) -> Result<bool, Refusal> {
    let bytes = admission.arithmetic(left.len().min(right.len()).checked_add(1))?;
    admission.units(bytes)?;
    Ok(left == right)
}
pub(super) fn values(
    left: &ValueId,
    right: &ValueId,
    admission: &mut Admission<'_>,
) -> Result<bool, Refusal> {
    let units =
        admission.arithmetic(left.as_str().len().min(right.as_str().len()).checked_add(2))?;
    admission.units(units)?; // one text equality plus retained fingerprint
    Ok(left == right)
}
