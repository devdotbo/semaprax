//! Canonical compiler-owned Vec type metadata only. Element acceptance, storage
//! and operation producers remain independently checked by receiving replay.
use super::*;
impl DeclarationIndex {
    pub(crate) fn append_vec_replay_metered(
        &mut self,
        physical: &mut ReplayPhysicalReservation,
        debit: &mut dyn FnMut(usize) -> Result<(), Refused>,
    ) -> Result<(), Refused> {
        let fail = || Refused::constructor("selected Vec metadata reconstruction refused");
        let id = crate::prelude::VEC_ID;
        for key in self.declarations.keys() {
            debit(
                key.as_str()
                    .len()
                    .checked_add(id.len())
                    .and_then(|n| n.checked_mul(3))
                    .ok_or_else(fail)?,
            )?;
            if key.as_str() == id {
                return Err(fail());
            }
        }
        for key in self.types_by_name.keys() {
            debit(
                key.len()
                    .checked_add(3)
                    .and_then(|n| n.checked_mul(3))
                    .ok_or_else(fail)?,
            )?;
            if key == "Vec" {
                return Err(fail());
            }
        }
        for key in self.type_parameters.keys().chain(self.record_fields.keys()) {
            debit(
                key.as_str()
                    .len()
                    .checked_add(id.len())
                    .and_then(|n| n.checked_mul(3))
                    .ok_or_else(fail)?,
            )?;
            if key.as_str() == id {
                return Err(fail());
            }
        }
        let bytes = crate::cleanup_plan::replay_btree_node_upper::<DeclarationId, Declaration>()
            .and_then(|n| {
                n.checked_add(crate::cleanup_plan::replay_btree_node_upper::<
                    String,
                    DeclarationId,
                >()?)
            })
            .and_then(|n| {
                n.checked_add(crate::cleanup_plan::replay_btree_node_upper::<
                    DeclarationId,
                    Vec<ResolvedTypeParameterDeclaration>,
                >()?)
            })
            .and_then(|n| {
                n.checked_add(crate::cleanup_plan::replay_btree_node_upper::<
                    DeclarationId,
                    Vec<ResolvedFieldDeclaration>,
                >()?)
            })
            .and_then(|n| n.checked_add(id.len().checked_mul(5)?))
            .and_then(|n| {
                n.checked_add(7 + std::mem::size_of::<ResolvedTypeParameterDeclaration>())
            })
            .ok_or_else(fail)?;
        debit(id.len() * 5 + 8)?;
        physical.reserve(bytes).map_err(|_| fail())?;
        // A fixed compiler identity: it and its four clones own no bytes. The
        // original identity text terms above stay reserved, unchanged, so the
        // cap and refusal order are those of the owned-text replay.
        let declaration = DeclarationId::compiler(crate::hir::CompilerDeclaration::Vec);
        self.declarations.insert(
            declaration.clone(),
            Declaration {
                id: declaration.clone(),
                name: "Vec".into(),
                kind: DeclarationKind::Record,
                identity_origin: IdentityOrigin::CompilerOwned,
                owner: None,
            },
        );
        self.types_by_name.insert("Vec".into(), declaration.clone());
        self.type_parameters.insert(
            declaration.clone(),
            vec![ResolvedTypeParameterDeclaration {
                name: "T".into(),
                index: 0,
                span: crate::ast::Span::default(),
            }],
        );
        self.record_fields.insert(declaration, Vec::new());
        Ok(())
    }
}
