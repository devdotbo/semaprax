//! Verbatim pre-checked borrowed-record oracle; test-only, never generic.
use super::*;
fn equal(
    a: &str,
    b: &str,
    take: &mut dyn FnMut(usize) -> Result<(), Vec<Diagnostic>>,
) -> Result<bool, Vec<Diagnostic>> {
    take(1)?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.bytes().zip(b.bytes()) {
        take(1)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
fn fields<'a>(
    records: &'a [hir::ResolvedTypeDeclaration],
    ty: &hir::ResolvedType,
    take: &mut dyn FnMut(usize) -> Result<(), Vec<Diagnostic>>,
) -> Result<Option<&'a [hir::ResolvedFieldDeclaration]>, Vec<Diagnostic>> {
    let hir::ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(None);
    };
    if !arguments.is_empty() {
        return Ok(None);
    }
    let mut selected = None;
    for record in records {
        if equal(record.id.as_str(), declaration.as_str(), take)?
            && selected.replace(record).is_some()
        {
            return Ok(None);
        }
    }
    let Some(record) = selected else {
        return Ok(None);
    };
    if !record.type_parameters.is_empty() {
        return Ok(None);
    }
    let hir::ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
        return Ok(None);
    };
    if !(1..=8).contains(&fields.len()) {
        return Ok(None);
    }
    for field in fields {
        take(1)?;
        if !hir::is_scalar_resolved_type(&field.ty)
            && !matches!(field.ty, ResolvedType::String | ResolvedType::Bytes)
        {
            return Ok(None);
        }
    }
    Ok(Some(fields))
}
pub(super) fn signature(
    function: &ResolvedFunction,
    records: &[hir::ResolvedTypeDeclaration],
    take: &mut dyn FnMut(usize) -> Result<(), Vec<Diagnostic>>,
) -> Result<bool, Vec<Diagnostic>> {
    if !hir::is_scalar_resolved_type(&function.return_type) {
        return Ok(false);
    }
    for parameter in &function.params {
        take(1)?;
        if hir::borrowed_scalar::parameter(&parameter.ty, parameter.ownership) {
            continue;
        }
        if parameter.ownership != OwnershipMode::Borrow
            || fields(records, &parameter.ty, take)?.is_none()
        {
            return Ok(false);
        }
    }
    Ok(true)
}
fn root(
    function: &ResolvedFunction,
    expression: &hir::ResolvedExpr,
    records: &[hir::ResolvedTypeDeclaration],
    take: &mut dyn FnMut(usize) -> Result<(), Vec<Diagnostic>>,
) -> Result<bool, Vec<Diagnostic>> {
    let ResolvedExprKind::Place(place) = &expression.kind else {
        return Ok(false);
    };
    if expression.ownership != OwnershipMode::Borrow
        || !place.projections.is_empty()
        || fields(records, &expression.ty, take)?.is_none()
    {
        return Ok(false);
    }
    let mut found = false;
    for parameter in &function.params {
        if equal(parameter.id.as_str(), place.root.as_str(), take)? {
            take(1)?;
            if found || parameter.ownership != OwnershipMode::Borrow {
                return Ok(false);
            }
            let same = match (&parameter.ty, &expression.ty) {
                (
                    ResolvedType::Nominal {
                        declaration: a,
                        arguments: aa,
                    },
                    ResolvedType::Nominal {
                        declaration: b,
                        arguments: bb,
                    },
                ) => {
                    take(1)?;
                    aa.is_empty() && bb.is_empty() && equal(a.as_str(), b.as_str(), take)?
                }
                _ => {
                    take(1)?;
                    false
                }
            };
            if !same {
                return Ok(false);
            }
            found = true;
        }
    }
    Ok(found)
}
// A direct scalar field read lowers to Place(Field), with the scalar type
// on the expression. Resolve the root's nominal type from the exact current
// Borrow parameter; the field expression cannot supply that authority.
fn projected_place(
    function: &ResolvedFunction,
    expression: &hir::ResolvedExpr,
    place: &hir::Place,
    records: &[hir::ResolvedTypeDeclaration],
    take: &mut dyn FnMut(usize) -> Result<(), Vec<Diagnostic>>,
) -> Result<bool, Vec<Diagnostic>> {
    take(1)?;
    let [hir::PlaceProjection::Field(field)] = place.projections.as_slice() else {
        return Ok(false);
    };
    if expression.ownership != OwnershipMode::Value || !hir::is_scalar_resolved_type(&expression.ty)
    {
        return Ok(false);
    }
    let mut found = None;
    for parameter in &function.params {
        if equal(parameter.id.as_str(), place.root.as_str(), take)?
            && found.replace(parameter).is_some()
        {
            return Ok(false);
        }
    }
    let Some(parameter) = found else {
        return Ok(false);
    };
    take(1)?;
    if parameter.ownership != OwnershipMode::Borrow {
        return Ok(false);
    }
    let Some(fields) = fields(records, &parameter.ty, take)? else {
        return Ok(false);
    };
    let mut selected = None;
    for current in fields {
        if equal(current.id.as_str(), field.as_str(), take)? && selected.replace(current).is_some()
        {
            return Ok(false);
        }
    }
    take(1)?;
    Ok(selected.is_some_and(|f| hir::is_scalar_resolved_type(&f.ty) && f.ty == expression.ty))
}
pub(super) fn expression(
    function: &ResolvedFunction,
    expression: &hir::ResolvedExpr,
    records: &[hir::ResolvedTypeDeclaration],
    take: &mut dyn FnMut(usize) -> Result<(), Vec<Diagnostic>>,
) -> Result<Option<bool>, Vec<Diagnostic>> {
    match &expression.kind {
        ResolvedExprKind::Place(place) if !place.projections.is_empty() => Ok(Some(
            projected_place(function, expression, place, records, take)?,
        )),
        ResolvedExprKind::Place(_) if matches!(expression.ty, ResolvedType::Nominal { .. }) => {
            Ok(Some(root(function, expression, records, take)?))
        }
        ResolvedExprKind::Project { base, field }
            if matches!(base.ty, ResolvedType::Nominal { .. }) =>
        {
            if !root(function, base, records, take)?
                || expression.ownership != OwnershipMode::Value
                || !hir::is_scalar_resolved_type(&expression.ty)
            {
                return Ok(Some(false));
            }
            let Some(fields) = fields(records, &base.ty, take)? else {
                return Ok(Some(false));
            };
            let mut selected = None;
            for current in fields {
                if equal(current.id.as_str(), field.as_str(), take)?
                    && selected.replace(current).is_some()
                {
                    return Ok(Some(false));
                }
            }
            Ok(Some(selected.is_some_and(|f| {
                hir::is_scalar_resolved_type(&f.ty) && f.ty == expression.ty
            })))
        }
        _ => Ok(None),
    }
}
