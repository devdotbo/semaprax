//! Defining-source helper shape only. Exact source/layout namespace and full
//! cleanup replay are independently required before the helper is retained.
use super::*;
fn equal<E>(a: &str, b: &str, take: &mut dyn FnMut(usize) -> Result<(), E>) -> Result<bool, E> {
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
fn fields<'a, E>(
    records: &'a [hir::ResolvedTypeDeclaration],
    ty: &hir::ResolvedType,
    take: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<Option<&'a [hir::ResolvedFieldDeclaration]>, E> {
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
pub(super) fn signature<E>(
    function: &ResolvedFunction,
    records: &[hir::ResolvedTypeDeclaration],
    take: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
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
fn root<E>(
    function: &ResolvedFunction,
    expression: &hir::ResolvedExpr,
    records: &[hir::ResolvedTypeDeclaration],
    take: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
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
fn projected_place<E>(
    function: &ResolvedFunction,
    expression: &hir::ResolvedExpr,
    place: &hir::Place,
    records: &[hir::ResolvedTypeDeclaration],
    take: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<bool, E> {
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
pub(super) fn expression<E>(
    function: &ResolvedFunction,
    expression: &hir::ResolvedExpr,
    records: &[hir::ResolvedTypeDeclaration],
    take: &mut dyn FnMut(usize) -> Result<(), E>,
) -> Result<Option<bool>, E> {
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direct_field_place_requires_current_borrow_root_scalar_field_and_exact_work() {
        let source = r#"module rows;
@id("rows.Row") record Row {@id("rows.Row.n") n:i64,@id("rows.Row.text") text:string,}
@id("rows.read") fn read(row:borrow Row)->i64 {row.n}
@id("rows.main") fn main()->i64 {0}
"#;
        let ast = crate::check(source, "rows.spx").unwrap();
        let program = hir::resolve(&ast).unwrap();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "rows.read")
            .unwrap();
        let ResolvedExprKind::Block { tail, .. } = &function.body.kind else {
            panic!("resolved block")
        };
        assert!(
            matches!(&tail.kind, ResolvedExprKind::Place(p) if matches!(p.projections.as_slice(), [hir::PlaceProjection::Field(_)]))
        );
        let mut spent = 0;
        let mut work = WORK;
        assert!(
            pure_records_metered(function, &program.types, &mut work, &mut |n| {
                spent += n;
                Ok(())
            })
            .unwrap()
        );
        assert_eq!(WORK - work, spent);
        for available in [spent, spent - 1] {
            let mut work = available;
            let result = pure_records_metered(function, &program.types, &mut work, &mut |_| Ok(()));
            if available == spent {
                assert!(result.unwrap());
                assert_eq!(work, 0);
            } else {
                assert!(result.is_err());
            }
        }
        for attack in 0..7 {
            let mut candidate = function.clone();
            let ResolvedExprKind::Block { tail, .. } = &mut candidate.body.kind else {
                unreachable!()
            };
            let ResolvedExprKind::Place(place) = &mut tail.kind else {
                unreachable!()
            };
            match attack {
                0 => place.root = hir::ValueId::intrinsic_parameter("foreign", 0),
                1 => {
                    place.projections[0] =
                        hir::PlaceProjection::Field(hir::DeclarationId::new("foreign.field"))
                }
                2 => tail.ty = ResolvedType::Bool,
                3 => tail.ownership = OwnershipMode::Own,
                4 => candidate.params[0].ownership = OwnershipMode::Own,
                5 => place.projections.push(place.projections[0].clone()),
                _ => {
                    candidate.params[0].ty = ResolvedType::Nominal {
                        declaration: hir::DeclarationId::new("foreign.Row"),
                        arguments: Vec::new(),
                    }
                }
            }
            assert!(
                !pure_records_metered(&candidate, &program.types, &mut { WORK }, &mut |_| Ok(()))
                    .unwrap(),
                "field place attack {attack}"
            );
        }
    }
}
