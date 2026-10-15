//! Actual immutable compiler AST suffix to complete canonical nominal payloads.
use super::extension_staging::{Cases, Fields, Type as StagedType};
use super::*;
use crate::ast::{Type, TypeDeclaration, TypeDeclarationKind};
use std::mem::size_of;
type Prior = [Option<ResolvedTypeDeclaration>; 9];
fn bytes<P: Payer, T>(payer: &mut P) -> Result<(), Failure<P::Error>> {
    payer.bytes(size_of::<T>()).map_err(Failure::Payer)
}
fn vector<T, P: Payer>(n: usize, payer: &mut P) -> Result<Vec<T>, Failure<P::Error>> {
    bytes::<P, Vec<T>>(payer)?;
    bytes::<P, Result<Vec<T>, Failure<P::Error>>>(payer)?;
    paid::vector(n, payer)
}
fn identity<P: Payer>(s: &str, payer: &mut P) -> Result<DeclarationId, Failure<P::Error>> {
    bytes::<P, DeclarationId>(payer)?;
    bytes::<P, Result<DeclarationId, P::Error>>(payer)?;
    bytes::<P, Result<DeclarationId, Failure<P::Error>>>(payer)?;
    paid::identity(s, payer)
}
fn field_type<P: Payer>(
    ty: &Type,
    source: &TypeDeclaration,
    owner: &DeclarationId,
    slot: usize,
    prior: &Prior,
    payer: &mut P,
) -> Result<StagedType, Failure<P::Error>> {
    bytes::<P, StagedType>(payer)?;
    bytes::<P, Option<ResolvedType>>(payer)?;
    payer.work(1).map_err(Failure::Payer)?;
    let Type::Named { name, arguments } = ty else {
        return Err(Failure::Schema);
    };
    if arguments.is_empty() {
        for (index, parameter) in source.type_parameters.iter().enumerate() {
            payer
                .work(
                    parameter
                        .name
                        .len()
                        .checked_add(name.len())
                        .ok_or(Failure::Layout)?,
                )
                .map_err(Failure::Payer)?;
            if parameter.name == *name {
                return Ok(StagedType(Some(ResolvedType::TypeParameter {
                    owner: owner.clone(),
                    index: u32::try_from(index).map_err(|_| Failure::Schema)?,
                })));
            }
        }
        return Err(Failure::Schema);
    }
    // The actual two compound compiler fields have one direct T argument.
    let [argument] = arguments.as_slice() else {
        return Err(Failure::Schema);
    };
    let Type::Named {
        arguments: inner, ..
    } = argument
    else {
        return Err(Failure::Schema);
    };
    if !inner.is_empty() {
        return Err(Failure::Schema);
    }
    let mut target = None;
    for (index, entry) in prior.iter().enumerate().take(slot) {
        let entry = entry.as_ref().ok_or(Failure::Schema)?;
        payer
            .work(
                entry
                    .name
                    .len()
                    .checked_add(name.len())
                    .ok_or(Failure::Layout)?,
            )
            .map_err(Failure::Payer)?;
        if entry.name == *name {
            target = Some(index);
            break;
        }
    }
    let declaration = prior[target.ok_or(Failure::Schema)?]
        .as_ref()
        .ok_or(Failure::Schema)?
        .id
        .clone();
    let value = field_type(argument, source, owner, slot, prior, payer)?;
    let mut arguments = vector(1, payer)?;
    arguments.push(value.take()); // no callback until the completed root is guarded
    Ok(StagedType(Some(ResolvedType::Nominal {
        declaration,
        arguments,
    })))
}
fn fields<P: Payer>(
    input: &[crate::ast::FieldDeclaration],
    source: &TypeDeclaration,
    owner: &DeclarationId,
    slot: usize,
    prior: &Prior,
    payer: &mut P,
) -> Result<Fields, Failure<P::Error>> {
    bytes::<P, Fields>(payer)?;
    bytes::<P, Option<ResolvedType>>(payer)?;
    let mut out = Fields(vector(input.len(), payer)?);
    for (index, field) in input.iter().enumerate() {
        payer.work(1).map_err(Failure::Payer)?;
        bytes::<P, ResolvedFieldDeclaration>(payer)?;
        let id = identity(&field.stable_id, payer)?;
        let name = VectorExtension::text(&field.name, payer)?;
        let ty = field_type(&field.ty, source, owner, slot, prior, payer)?;
        out.push(ResolvedFieldDeclaration {
            id,
            name,
            index: u32::try_from(index).map_err(|_| Failure::Schema)?,
            ty: ty.take(),
            span: field.span,
        });
    }
    Ok(out)
}
pub(super) fn acquire<P: Payer>(
    slot: usize,
    prior: &Prior,
    payer: &mut P,
) -> Result<ResolvedTypeDeclaration, Failure<P::Error>> {
    let source = crate::prelude::declarations()
        .get(slot.checked_add(2).ok_or(Failure::Layout)?)
        .ok_or(Failure::Schema)?;
    payer
        .bytes(
            2 * size_of::<ResolvedTypeDeclaration>()
                + size_of::<Option<ResolvedTypeDeclaration>>()
                + size_of::<Result<ResolvedTypeDeclaration, Failure<P::Error>>>(),
        )
        .map_err(Failure::Payer)?;
    payer.work(1).map_err(Failure::Payer)?;
    payer.work(1).map_err(Failure::Payer)?;
    let record = match &source.kind {
        TypeDeclarationKind::Record { fields } if fields.is_empty() => true,
        TypeDeclarationKind::Variant { .. } => false,
        _ => return Err(Failure::Schema),
    };
    payer.work(1).map_err(Failure::Payer)?;
    // Current finite schema has at most K,V; source authority remains pointer bound.
    if source.type_parameters.len() > 2 {
        return Err(Failure::Schema);
    }
    let id = identity(&source.stable_id, payer)?;
    let name = VectorExtension::text(&source.name, payer)?;
    let mut parameters = vector(source.type_parameters.len(), payer)?;
    for (index, parameter) in source.type_parameters.iter().enumerate() {
        payer.work(1).map_err(Failure::Payer)?;
        bytes::<P, ResolvedTypeParameterDeclaration>(payer)?;
        parameters.push(ResolvedTypeParameterDeclaration {
            name: VectorExtension::text(&parameter.name, payer)?,
            index: u32::try_from(index).map_err(|_| Failure::Schema)?,
            span: parameter.span,
        });
    }
    bytes::<P, Result<NominalPayload<ResolvedTypeParameterDeclaration>, Failure<P::Error>>>(payer)?;
    let type_parameters = if parameters.is_empty() {
        parameters.into()
    } else {
        NominalPayload::from_funded_vec(&mut parameters, |n| payer.bytes(n))
            .map_err(Failure::Payer)?
    };
    let kind = if record {
        bytes::<P, Vec<ResolvedFieldDeclaration>>(payer)?;
        bytes::<P, NominalPayload<ResolvedFieldDeclaration>>(payer)?;
        ResolvedTypeDeclarationKind::Record {
            fields: Vec::new().into(),
        }
    } else {
        let TypeDeclarationKind::Variant { cases: input } = &source.kind else {
            unreachable!()
        };
        bytes::<P, Cases>(payer)?;
        bytes::<P, Option<ResolvedType>>(payer)?;
        let mut cases = Cases(vector(input.len(), payer)?);
        for (index, case) in input.iter().enumerate() {
            payer.work(1).map_err(Failure::Payer)?;
            bytes::<P, ResolvedVariantCaseDeclaration>(payer)?;
            let case_id = identity(&case.stable_id, payer)?;
            let case_name = VectorExtension::text(&case.name, payer)?;
            let fields = fields(&case.fields, source, &id, slot, prior, payer)?;
            cases.push(ResolvedVariantCaseDeclaration {
                id: case_id,
                name: case_name,
                index: u32::try_from(index).map_err(|_| Failure::Schema)?,
                fields: fields.take(),
                span: case.span,
            });
        }
        bytes::<P, Result<NominalPayload<ResolvedVariantCaseDeclaration>, Failure<P::Error>>>(
            payer,
        )?;
        let cases = NominalPayload::from_funded_vec(&mut cases.0, |n| payer.bytes(n))
            .map_err(Failure::Payer)?;
        ResolvedTypeDeclarationKind::Variant { cases }
    };
    Ok(ResolvedTypeDeclaration {
        id,
        name,
        type_parameters,
        kind,
        span: source.span,
    })
}
