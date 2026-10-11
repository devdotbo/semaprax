//! Canonical compiler-owned Vec type metadata only. Element acceptance, storage
//! and operation producers remain independently checked by receiving replay.
use super::*;
impl DeclarationIndex {
    pub(crate) fn append_vec_replay_metered(
        &mut self,
        physical: &mut ReplayPhysicalReservation,
        debit: &mut dyn FnMut(usize) -> Result<(), crate::diagnostic::Diagnostic>,
    ) -> Result<(), crate::diagnostic::Diagnostic> {
        let fail = || {
            crate::diagnostic::Diagnostic::io(
                "SPX-H006",
                "selected Vec metadata reconstruction refused",
            )
        };
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
        self.declarations.insert(
            DeclarationId::new(id),
            Declaration {
                id: DeclarationId::new(id),
                name: "Vec".into(),
                kind: DeclarationKind::Record,
                identity_origin: IdentityOrigin::CompilerOwned,
                owner: None,
            },
        );
        self.types_by_name
            .insert("Vec".into(), DeclarationId::new(id));
        self.type_parameters.insert(
            DeclarationId::new(id),
            vec![ResolvedTypeParameterDeclaration {
                name: "T".into(),
                index: 0,
                span: crate::ast::Span::default(),
            }],
        );
        self.record_fields
            .insert(DeclarationId::new(id), Vec::new());
        Ok(())
    }
}
