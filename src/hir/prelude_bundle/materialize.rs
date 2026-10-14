//! Existing byte-reservation seam for returned HIR headers. Full source/type
//! authentication remains at the resolver root; an ID miss substitutes nothing.
use super::*;

impl BasePreludeBundle {
    pub(crate) fn materialize_type<E>(
        &self,
        id: &DeclarationId,
        mut reserve: impl FnMut(usize) -> Result<(), E>,
    ) -> Result<Option<ResolvedTypeDeclaration>, E> {
        let Some(ty) = self.types_slice().iter().find(|ty| ty.id == *id) else {
            return Ok(None);
        };
        // Only names get new heap backing. All nominal sequences and IDs
        // reference their already-funded immutable owners. Include separately
        // live initialization, optional and receiving result carriers.
        let bytes = ty
            .name
            .len()
            .checked_add(std::mem::size_of::<String>())
            .and_then(|n| n.checked_add(std::mem::size_of::<ResolvedTypeDeclaration>()))
            .and_then(|n| n.checked_add(std::mem::size_of::<Option<ResolvedTypeDeclaration>>()))
            .and_then(|n| {
                n.checked_add(std::mem::size_of::<
                    Result<Option<ResolvedTypeDeclaration>, E>,
                >())
            })
            .expect("fixed base type materialization has addressable layouts");
        reserve(bytes)?;
        let mut name = String::with_capacity(ty.name.len());
        assert_eq!(
            name.capacity(),
            ty.name.len(),
            "exact base type name backing"
        );
        name.push_str(&ty.name);
        let ResolvedTypeDeclarationKind::Variant { cases } = &ty.kind else {
            unreachable!("immutable base schema only publishes variants");
        };
        Ok(Some(ResolvedTypeDeclaration {
            id: ty.id.clone(),
            name,
            type_parameters: ty.type_parameters.clone(),
            kind: ResolvedTypeDeclarationKind::Variant {
                cases: cases.clone(),
            },
            span: ty.span,
        }))
    }
}
