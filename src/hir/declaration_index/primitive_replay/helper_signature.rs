//! Imported scalar helper signatures use only independently reconstructed
//! record metadata; generic or owning nominal helper signatures remain closed.
use super::*;
pub(super) fn admitted(
    index: &DeclarationIndex,
    function: &ResolvedFunction,
    debit: &mut dyn FnMut(usize) -> Result<(), Refused>,
) -> Result<bool, Refused> {
    let fail = || Refused::constructor("scalar helper record signature work overflow");
    if !is_scalar_resolved_type(&function.return_type) {
        return Ok(false);
    }
    for parameter in &function.params {
        if crate::hir::borrowed_scalar::parameter(&parameter.ty, parameter.ownership) {
            continue;
        }
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = &parameter.ty
        else {
            return Ok(false);
        };
        if parameter.ownership != OwnershipMode::Borrow || !arguments.is_empty() {
            return Ok(false);
        }
        // Prepay exact retained keys before both tree probes. No whole prelude
        // or argument-backed declaration can be manufactured by this receiver.
        for key in index.declarations.keys() {
            debit(
                key.as_str()
                    .len()
                    .checked_add(declaration.as_str().len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(fail)?,
            )?;
        }
        for key in index.record_fields.keys() {
            debit(
                key.as_str()
                    .len()
                    .checked_add(declaration.as_str().len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(fail)?,
            )?;
        }
        let Some(record) = index.declarations.get(declaration) else {
            return Ok(false);
        };
        let Some(fields) = index.record_fields.get(declaration) else {
            return Ok(false);
        };
        if record.kind != DeclarationKind::Record
            || record.identity_origin == IdentityOrigin::CompilerOwned
            || record.owner.is_some()
            || !(1..=8).contains(&fields.len())
        {
            return Ok(false);
        }
        for field in fields {
            debit(1)?;
            if !is_scalar_resolved_type(&field.ty)
                && !matches!(field.ty, ResolvedType::String | ResolvedType::Bytes)
            {
                return Ok(false);
            }
        }
    }
    Ok(true)
}
