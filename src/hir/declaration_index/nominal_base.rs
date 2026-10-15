//! Trusted current compiler prelude coordinates, not ID-based source authority.
use super::*;
use crate::hir::prelude_bundle::BasePreludeBundle;
use std::mem::size_of;
impl DeclarationIndex {
    pub(super) fn base_type_for_source<'a>(
        base: Option<&'a BasePreludeBundle>,
        prelude: &[crate::ast::TypeDeclaration],
        declaration: &crate::ast::TypeDeclaration,
    ) -> Option<&'a ResolvedTypeDeclaration> {
        let base = base?;
        let current = crate::prelude::declarations();
        // Only the exact compiler-owned source pair may use schema payloads.
        // Authored or decoded declarations with matching IDs substitute nothing.
        if prelude.len() < 2 || !std::ptr::eq(&prelude[..2], &current[..2]) {
            return None;
        }
        let ordinal = prelude[..2]
            .iter()
            .position(|item| std::ptr::eq(item, declaration))?;
        base.types_slice().get(ordinal)
    }
    pub(super) fn compiler_type_for_source<'a>(
        base: Option<&'a BasePreludeBundle>,
        vector: Option<&'a crate::hir::prelude_bundle::VectorExtension>,
        prelude: &[crate::ast::TypeDeclaration],
        declaration: &crate::ast::TypeDeclaration,
    ) -> Option<&'a ResolvedTypeDeclaration> {
        Self::base_type_for_source(base, prelude, declaration).or_else(|| {
            crate::hir::prelude_bundle::VectorExtension::for_source(vector, prelude, declaration)
        })
    }
    pub(super) fn reserve_base_receivers<E>(
        base: &BasePreludeBundle,
        reserve: &mut impl FnMut(usize) -> Result<(), E>,
    ) -> Result<(), E> {
        fn node<K, V>() -> usize {
            crate::cleanup_plan::replay_btree_node_upper::<K, V>()
                .expect("fixed base receiving map layout")
        }
        // Full new metadata map backing and simultaneous initializer/receivers;
        // unchanged enclosing forecast receives no subtraction or payload credit.
        for declaration in base.declarations_slice() {
            let names = 2 * declaration.name.len() + 2 * size_of::<String>();
            let map = if declaration.owner.is_none() {
                node::<String, DeclarationId>()
            } else {
                node::<(DeclarationId, String), DeclarationId>()
            };
            reserve(
                names + map + node::<DeclarationId, Declaration>() + 2 * size_of::<Declaration>(),
            )?;
        }
        for _ in base.types_slice() {
            reserve(
                2 * size_of::<ResolvedTypeDeclaration>()
                    + size_of::<Result<ResolvedTypeDeclaration, Diagnostic>>()
                    + 2 * size_of::<NominalPayload<ResolvedTypeParameterDeclaration>>()
                    + 2 * size_of::<NominalPayload<ResolvedVariantCaseDeclaration>>(),
            )?;
        }
        Ok(())
    }
    pub(super) fn insert_base_variant(&mut self, ty: &ResolvedTypeDeclaration) {
        let ResolvedTypeDeclarationKind::Variant { cases } = &ty.kind else {
            unreachable!("sealed base variant")
        };
        for case in cases {
            self.insert_case(
                ty.id.clone(),
                case.name.clone(),
                case.id.clone(),
                IdentityOrigin::CompilerOwned,
            );
            for field in &case.fields {
                self.fields_by_owner_name
                    .insert((case.id.clone(), field.name.clone()), field.id.clone());
                self.declarations.insert(
                    field.id.clone(),
                    Declaration {
                        id: field.id.clone(),
                        name: field.name.clone(),
                        kind: DeclarationKind::CaseField,
                        identity_origin: IdentityOrigin::CompilerOwned,
                        owner: Some(case.id.clone()),
                    },
                );
            }
        }
        self.variant_cases.insert(ty.id.clone(), cases.clone());
    }
}
