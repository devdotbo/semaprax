//! Exact first extension prototype: one actual compiler Vec parameter owner.
//! No authored schema, process cache, inferred lookup authority, or base work.
use super::*;
use crate::ast::TypeDeclarationKind;
use std::mem::size_of;

#[derive(Debug)]
pub(crate) struct VectorExtension {
    pub(crate) ty: ResolvedTypeDeclaration,
}
impl VectorExtension {
    fn text<P: Payer>(value: &str, payer: &mut P) -> Result<String, Failure<P::Error>> {
        // Separate actual helper initializer/conversion/receiving carriers;
        // String backing and its copy/validation work stay with paid::string.
        payer
            .bytes(
                size_of::<String>()
                    + size_of::<Vec<u8>>()
                    + size_of::<Result<String, std::string::FromUtf8Error>>()
                    + size_of::<Result<String, Failure<P::Error>>>(),
            )
            .map_err(Failure::Payer)?;
        paid::string(value, payer)
    }
    pub(super) fn acquire<P: Payer>(payer: &mut P) -> Result<Self, Failure<P::Error>> {
        payer
            .bytes(
                2 * size_of::<Self>()
                    + size_of::<Option<Self>>()
                    + size_of::<Result<Self, Failure<P::Error>>>()
                    + size_of::<ResolvedTypeParameterDeclaration>()
                    + size_of::<Vec<ResolvedTypeParameterDeclaration>>()
                    + size_of::<Result<Vec<ResolvedTypeParameterDeclaration>, Failure<P::Error>>>()
                    + 2 * size_of::<DeclarationId>()
                    + size_of::<Result<DeclarationId, P::Error>>(),
            )
            .map_err(Failure::Payer)?;
        payer.work(1).map_err(Failure::Payer)?;
        let source = &crate::prelude::declarations()[2];
        payer.work(1).map_err(Failure::Payer)?;
        if !matches!(&source.kind, TypeDeclarationKind::Record { fields } if fields.is_empty()) {
            return Err(Failure::Schema);
        }
        payer.work(1).map_err(Failure::Payer)?;
        let [parameter] = source.type_parameters.as_slice() else {
            return Err(Failure::Schema);
        };
        let id = paid::identity(&source.stable_id, payer)?;
        let name = Self::text(&source.name, payer)?;
        let mut parameters = paid::vector(1, payer)?;
        payer.work(1).map_err(Failure::Payer)?;
        parameters.push(ResolvedTypeParameterDeclaration {
            name: Self::text(&parameter.name, payer)?,
            index: 0,
            span: parameter.span,
        });
        let type_parameters = NominalPayload::from_funded_vec(&mut parameters, |n| payer.bytes(n))
            .map_err(Failure::Payer)?;
        Ok(Self {
            ty: ResolvedTypeDeclaration {
                id,
                name,
                type_parameters,
                kind: ResolvedTypeDeclarationKind::Record {
                    fields: Vec::new().into(),
                },
                span: source.span,
            },
        })
    }
    pub(crate) fn for_source<'a>(
        value: Option<&'a Self>,
        prelude: &[crate::ast::TypeDeclaration],
        source: &crate::ast::TypeDeclaration,
    ) -> Option<&'a ResolvedTypeDeclaration> {
        let current = crate::prelude::declarations();
        if !matches!(prelude.len(), 3 | 4 | 6 | 8 | 9 | 11)
            || !std::ptr::eq(prelude, &current[..prelude.len()])
            || !std::ptr::eq(source, &current[2])
        {
            return None;
        }
        Some(&value?.ty)
    }
}

#[cfg(test)]
#[path = "vector_extension/tests.rs"]
mod tests;
