//! Schema-driven base producer. Every Vec/String/identity/shared owner is paid
//! before construction; partial refusal drops staged owners without publication.
use super::*;

fn count() -> Option<usize> {
    BASE.iter().try_fold(BASE.len(), |count, schema| {
        schema.cases.iter().try_fold(count, |count, case| {
            count.checked_add(1)?.checked_add(case.fields.len())
        })
    })
}

fn descriptor<P: Payer>(
    text: &str,
    name: &str,
    kind: DeclarationKind,
    owner: Option<DeclarationId>,
    payer: &mut P,
) -> Result<Declaration, Failure<P::Error>> {
    Ok(Declaration {
        id: paid::identity(text, payer)?,
        name: paid::string(name, payer)?,
        kind,
        identity_origin: IdentityOrigin::CompilerOwned,
        owner,
    })
}

fn find<P: Payer>(
    declarations: &[Declaration],
    text: &str,
    payer: &mut P,
) -> Result<DeclarationId, Failure<P::Error>> {
    for declaration in declarations {
        payer
            .work(
                declaration
                    .id
                    .as_str()
                    .len()
                    .checked_add(text.len())
                    .ok_or(Failure::Layout)?,
            )
            .map_err(Failure::Payer)?;
        if declaration.id.as_str() == text {
            return Ok(declaration.id.clone());
        }
    }
    Err(Failure::Schema)
}

fn nominal<P: Payer>(
    schema: &Variant,
    declarations: &[Declaration],
    payer: &mut P,
) -> Result<ResolvedTypeDeclaration, Failure<P::Error>> {
    let owner = find(declarations, schema.id, payer)?;
    let mut parameters = paid::vector(schema.parameters.len(), payer)?;
    for (index, name) in schema.parameters.iter().enumerate() {
        payer.work(1).map_err(Failure::Payer)?;
        parameters.push(ResolvedTypeParameterDeclaration {
            name: paid::string(name, payer)?,
            index: u32::try_from(index).map_err(|_| Failure::Schema)?,
            span: Span::default(),
        });
    }
    let parameters = NominalPayload::from_funded_vec(&mut parameters, |n| payer.bytes(n))
        .map_err(Failure::Payer)?;
    let mut cases = paid::vector(schema.cases.len(), payer)?;
    for (index, case) in schema.cases.iter().enumerate() {
        let mut fields = paid::vector(case.fields.len(), payer)?;
        for (index, field) in case.fields.iter().enumerate() {
            let mut parameter = None;
            for (ordinal, name) in schema.parameters.iter().enumerate() {
                payer
                    .work(
                        name.len()
                            .checked_add(field.parameter.len())
                            .ok_or(Failure::Layout)?,
                    )
                    .map_err(Failure::Payer)?;
                if *name == field.parameter {
                    parameter = Some(u32::try_from(ordinal).map_err(|_| Failure::Schema)?);
                    break;
                }
            }
            fields.push(ResolvedFieldDeclaration {
                id: find(declarations, field.id, payer)?,
                name: paid::string(field.name, payer)?,
                index: u32::try_from(index).map_err(|_| Failure::Schema)?,
                ty: ResolvedType::TypeParameter {
                    owner: owner.clone(),
                    index: parameter.ok_or(Failure::Schema)?,
                },
                span: Span::default(),
            });
        }
        cases.push(ResolvedVariantCaseDeclaration {
            id: find(declarations, case.id, payer)?,
            name: paid::string(case.name, payer)?,
            index: u32::try_from(index).map_err(|_| Failure::Schema)?,
            fields,
            span: Span::default(),
        });
    }
    let cases =
        NominalPayload::from_funded_vec(&mut cases, |n| payer.bytes(n)).map_err(Failure::Payer)?;
    Ok(ResolvedTypeDeclaration {
        id: owner,
        name: paid::string(schema.name, payer)?,
        type_parameters: parameters,
        kind: ResolvedTypeDeclarationKind::Variant { cases },
        span: Span::default(),
    })
}

pub(super) fn contents<P: Payer>(payer: &mut P) -> Result<Contents, Failure<P::Error>> {
    payer.work(1).map_err(Failure::Payer)?;
    let mut declarations = paid::vector(count().ok_or(Failure::Layout)?, payer)?;
    for schema in BASE {
        let top = descriptor(
            schema.id,
            schema.name,
            DeclarationKind::Variant,
            None,
            payer,
        )?;
        let owner = top.id.clone();
        declarations.push(top);
        for case in schema.cases {
            let child = descriptor(
                case.id,
                case.name,
                DeclarationKind::VariantCase,
                Some(owner.clone()),
                payer,
            )?;
            let case_owner = child.id.clone();
            declarations.push(child);
            for field in case.fields {
                declarations.push(descriptor(
                    field.id,
                    field.name,
                    DeclarationKind::CaseField,
                    Some(case_owner.clone()),
                    payer,
                )?);
            }
        }
    }
    // Fallible insertion sort performs no hidden allocation and charges each
    // actual key comparison before reading its bytes. IDs must remain unique.
    for i in 1..declarations.len() {
        let mut j = i;
        while j > 0 {
            payer
                .work(
                    declarations[j - 1]
                        .id
                        .as_str()
                        .len()
                        .checked_add(declarations[j].id.as_str().len())
                        .ok_or(Failure::Layout)?,
                )
                .map_err(Failure::Payer)?;
            match declarations[j - 1].id.cmp(&declarations[j].id) {
                std::cmp::Ordering::Equal => return Err(Failure::Schema),
                std::cmp::Ordering::Less => break,
                std::cmp::Ordering::Greater => declarations.swap(j - 1, j),
            }
            j -= 1;
        }
    }
    let mut types = paid::vector(BASE.len(), payer)?;
    for schema in BASE {
        types.push(nominal(schema, &declarations, payer)?);
    }
    Ok(Contents {
        declarations,
        types,
    })
}
