//! Narrow compiler-owned Option metadata; never constructs the full prelude.
use super::*;
impl DeclarationIndex {
    pub(crate) fn append_option_u8_replay_metered(
        &mut self,
        physical: &mut ReplayPhysicalReservation,
        debit: &mut dyn FnMut(usize) -> Result<(), Refused>,
    ) -> Result<(), Refused> {
        let fail = || Refused::constructor("selected Option metadata reconstruction refused");
        // These maps start empty on the primitive route. No unbounded tree
        // comparison or candidate metadata is copied into compiler authority.
        if !self.variant_cases.is_empty() || !self.case_fields.is_empty() {
            return Err(fail());
        }
        // The only admitted parameter entries are the empty lists of replayed
        // flat source records: a declared non-compiler record with 1..=8
        // retained fields, each declared under it. Generic, compiler-owned,
        // orphaned or unfielded entries keep Option authority closed. The
        // first debit also covers the later Option parameter insertion.
        for (owner, parameters) in &self.type_parameters {
            debit(
                owner
                    .as_str()
                    .len()
                    .checked_add(crate::prelude::OPTION_ID.len())
                    .and_then(|n| n.checked_mul(3))
                    .ok_or_else(fail)?,
            )?;
            let mut declared = false;
            for (key, declaration) in &self.declarations {
                debit(
                    key.as_str()
                        .len()
                        .checked_add(owner.as_str().len())
                        .ok_or_else(fail)?,
                )?;
                declared |= key == owner
                    && declaration.id == *owner
                    && declaration.kind == DeclarationKind::Record
                    && declaration.identity_origin != IdentityOrigin::CompilerOwned
                    && declaration.owner.is_none();
            }
            let mut retained = None;
            for (key, fields) in &self.record_fields {
                debit(
                    key.as_str()
                        .len()
                        .checked_add(owner.as_str().len())
                        .ok_or_else(fail)?,
                )?;
                if key == owner {
                    retained = Some(fields);
                }
            }
            let fields = retained.ok_or_else(fail)?;
            if !parameters.is_empty() || !declared || !(1..=8).contains(&fields.len()) {
                return Err(fail());
            }
            for field in fields {
                let mut owned = false;
                for (key, declaration) in &self.declarations {
                    debit(
                        key.as_str()
                            .len()
                            .checked_add(field.id.as_str().len())
                            .ok_or_else(fail)?,
                    )?;
                    owned |= key == &field.id
                        && declaration.kind == DeclarationKind::Field
                        && declaration.identity_origin != IdentityOrigin::CompilerOwned
                        && declaration.owner.as_ref() == Some(owner);
                }
                if !owned {
                    return Err(fail());
                }
            }
        }
        // Every identity below is a fixed compiler identity and owns no bytes.
        // Work debits and physical reservations keep their original identity
        // text terms unchanged, so caps and refusal order are those of the
        // owned-text replay; those terms are conservative, not identity payment.
        use crate::hir::CompilerDeclaration as Id;
        let specs = [
            (Id::Option, "Option", DeclarationKind::Variant, None),
            (
                Id::OptionNone,
                "None",
                DeclarationKind::VariantCase,
                Some(Id::Option),
            ),
            (
                Id::OptionSome,
                "Some",
                DeclarationKind::VariantCase,
                Some(Id::Option),
            ),
            (
                Id::OptionSomeValue,
                "value",
                DeclarationKind::CaseField,
                Some(Id::OptionSome),
            ),
        ];
        for (identity, name, kind, owner) in specs {
            let id = identity.text();
            let owner_text = owner.map(Id::text);
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
            let bytes =
                crate::cleanup_plan::replay_btree_node_upper::<DeclarationId, Declaration>()
                    .and_then(|n| n.checked_add(id.len().checked_mul(2)?))
                    .and_then(|n| n.checked_add(name.len()))
                    .and_then(|n| n.checked_add(owner_text.map_or(0, str::len)))
                    .ok_or_else(fail)?;
            debit(bytes)?;
            physical.reserve(bytes).map_err(|_| fail())?;
            self.declarations.insert(
                DeclarationId::compiler(identity),
                Declaration {
                    id: DeclarationId::compiler(identity),
                    name: name.into(),
                    kind,
                    identity_origin: IdentityOrigin::CompilerOwned,
                    owner: owner.map(DeclarationId::compiler),
                },
            );
        }
        let parameter = crate::cleanup_plan::replay_btree_node_upper::<
            DeclarationId,
            Vec<ResolvedTypeParameterDeclaration>,
        >()
        .and_then(|n| n.checked_add(crate::prelude::OPTION_ID.len()))
        .and_then(|n| n.checked_add(std::mem::size_of::<ResolvedTypeParameterDeclaration>() + 1))
        .ok_or_else(fail)?;
        debit(parameter)?;
        physical.reserve(parameter).map_err(|_| fail())?;
        self.type_parameters.insert(
            DeclarationId::compiler(Id::Option),
            vec![ResolvedTypeParameterDeclaration {
                name: "T".into(),
                index: 0,
                span: crate::ast::Span::default(),
            }],
        );
        let field_bytes = std::mem::size_of::<ResolvedFieldDeclaration>()
            + crate::prelude::OPTION_SOME_VALUE_ID.len()
            + "value".len()
            + crate::prelude::OPTION_ID.len();
        let cases_bytes = crate::cleanup_plan::replay_btree_node_upper::<
            DeclarationId,
            Vec<ResolvedVariantCaseDeclaration>,
        >()
        .and_then(|n| n.checked_add(crate::prelude::OPTION_ID.len()))
        .and_then(|n| n.checked_add(2 * std::mem::size_of::<ResolvedVariantCaseDeclaration>()))
        .and_then(|n| n.checked_add(crate::prelude::OPTION_NONE_ID.len() + 4))
        .and_then(|n| n.checked_add(crate::prelude::OPTION_SOME_ID.len() + 4))
        .and_then(|n| n.checked_add(field_bytes))
        .ok_or_else(fail)?;
        debit(cases_bytes)?;
        physical.reserve(cases_bytes).map_err(|_| fail())?;
        let field = || ResolvedFieldDeclaration {
            id: DeclarationId::compiler(Id::OptionSomeValue),
            name: "value".into(),
            index: 0,
            ty: ResolvedType::TypeParameter {
                owner: DeclarationId::compiler(Id::Option),
                index: 0,
            },
            span: crate::ast::Span::default(),
        };
        let cases = vec![
            ResolvedVariantCaseDeclaration {
                id: DeclarationId::compiler(Id::OptionNone),
                name: "None".into(),
                index: 0,
                fields: Vec::new(),
                span: crate::ast::Span::default(),
            },
            ResolvedVariantCaseDeclaration {
                id: DeclarationId::compiler(Id::OptionSome),
                name: "Some".into(),
                index: 1,
                fields: vec![field()],
                span: crate::ast::Span::default(),
            },
        ];
        self.variant_cases
            .insert(DeclarationId::compiler(Id::Option), cases);
        for (id, some) in [(Id::OptionNone, false), (Id::OptionSome, true)] {
            let bytes = crate::cleanup_plan::replay_btree_node_upper::<
                DeclarationId,
                Vec<ResolvedFieldDeclaration>,
            >()
            .and_then(|n| n.checked_add(id.text().len()))
            .and_then(|n| n.checked_add(if some { field_bytes } else { 0 }))
            .ok_or_else(fail)?;
            debit(bytes)?;
            physical.reserve(bytes).map_err(|_| fail())?;
            self.case_fields.insert(
                DeclarationId::compiler(id),
                if some { vec![field()] } else { Vec::new() },
            );
        }
        Ok(())
    }
}
mod tests;
