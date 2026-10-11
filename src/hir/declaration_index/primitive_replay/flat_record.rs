//! Minimal record lookup reconstructed only after private source-layout replay.
//! No type-key, generic, inheritance or import caches are materialized.
use super::*;
impl DeclarationIndex {
    #[cfg(test)]
    pub(crate) fn append_flat_record_replay(
        &mut self,
        record: &ResolvedTypeDeclaration,
        origin: IdentityOrigin,
        field_origin: impl Fn(usize) -> Option<IdentityOrigin>,
        physical: &mut ReplayPhysicalReservation,
    ) -> Result<(), crate::diagnostic::Diagnostic> {
        self.append_flat_record_replay_metered(record, origin, field_origin, physical, &mut |_| {
            Ok(())
        })
    }
    pub(crate) fn append_flat_record_replay_metered(
        &mut self,
        record: &ResolvedTypeDeclaration,
        origin: IdentityOrigin,
        field_origin: impl Fn(usize) -> Option<IdentityOrigin>,
        physical: &mut ReplayPhysicalReservation,
        debit: &mut dyn FnMut(usize) -> Result<(), crate::diagnostic::Diagnostic>,
    ) -> Result<(), crate::diagnostic::Diagnostic> {
        let fail = || {
            crate::diagnostic::Diagnostic::io(
                "SPX-H006",
                "flat-record declaration reconstruction refused",
            )
        };
        let ResolvedTypeDeclarationKind::Record { fields } = &record.kind else {
            return Err(fail());
        };
        debit(1)?;
        // Visit retained keys before the map operations; the three debits cover
        // duplicate discovery, lookup and insertion comparisons independently.
        for key in self.declarations.keys() {
            debit(
                key.as_str()
                    .len()
                    .checked_add(record.id.as_str().len())
                    .and_then(|n| n.checked_mul(3))
                    .ok_or_else(fail)?,
            )?;
        }
        for key in self.types_by_name.keys() {
            debit(
                key.len()
                    .checked_add(record.name.len())
                    .and_then(|n| n.checked_mul(3))
                    .ok_or_else(fail)?,
            )?;
        }
        if !record.type_parameters.is_empty()
            || !(1..=8).contains(&fields.len())
            || self.declarations.contains_key(&record.id)
            || self.types_by_name.contains_key(&record.name)
        {
            return Err(fail());
        }
        for (ordinal, field) in fields.iter().enumerate() {
            debit(1)?;
            for key in self.declarations.keys() {
                debit(
                    key.as_str()
                        .len()
                        .checked_add(field.id.as_str().len())
                        .and_then(|n| n.checked_mul(3))
                        .ok_or_else(fail)?,
                )?;
            }
            for old in &fields[..ordinal] {
                debit(
                    old.id
                        .as_str()
                        .len()
                        .checked_add(field.id.as_str().len())
                        .and_then(|n| n.checked_add(old.name.len()))
                        .and_then(|n| n.checked_add(field.name.len()))
                        .ok_or_else(fail)?,
                )?;
            }
            if field.index as usize != ordinal
                || field_origin(ordinal).is_none()
                || self.declarations.contains_key(&field.id)
                || field.id == record.id
                || fields[..ordinal]
                    .iter()
                    .any(|old| old.id == field.id || old.name == field.name)
                || !(is_scalar_resolved_type(&field.ty)
                    || matches!(field.ty, ResolvedType::String | ResolvedType::Bytes))
            {
                return Err(fail());
            }
        }
        let node = crate::cleanup_plan::replay_btree_node_upper::<DeclarationId, Declaration>()
            .ok_or_else(fail)?;
        let names = crate::cleanup_plan::replay_btree_node_upper::<String, DeclarationId>()
            .ok_or_else(fail)?;
        let record_fields = crate::cleanup_plan::replay_btree_node_upper::<
            DeclarationId,
            Vec<ResolvedFieldDeclaration>,
        >()
        .ok_or_else(fail)?;
        let roots = record
            .id
            .as_str()
            .len()
            .checked_mul(4)
            .and_then(|n| {
                record
                    .name
                    .len()
                    .checked_mul(2)
                    .and_then(|m| n.checked_add(m))
            })
            .and_then(|n| n.checked_add(node))
            .and_then(|n| n.checked_add(names))
            .and_then(|n| n.checked_add(record_fields))
            .and_then(|n| {
                fields
                    .len()
                    .checked_mul(std::mem::size_of::<ResolvedFieldDeclaration>())
                    .and_then(|m| n.checked_add(m))
            })
            .ok_or_else(fail)?;
        let bytes = fields
            .iter()
            .try_fold(roots, |sum, field| {
                // Tree key + declaration ID + retained field ID. Owner and display
                // name appear in the declaration; the retained field copies name.
                sum.checked_add(node)?
                    .checked_add(field.id.as_str().len().checked_mul(3)?)?
                    .checked_add(record.id.as_str().len())?
                    .checked_add(field.name.len().checked_mul(2)?)
            })
            .ok_or_else(fail)?;
        debit(bytes)?;
        physical.reserve(bytes).map_err(|_| fail())?;
        self.declarations.insert(
            record.id.clone(),
            Declaration {
                id: record.id.clone(),
                name: record.name.clone(),
                kind: DeclarationKind::Record,
                identity_origin: origin,
                owner: None,
            },
        );
        self.types_by_name
            .insert(record.name.clone(), record.id.clone());
        self.record_fields.insert(record.id.clone(), fields.clone());
        for (ordinal, field) in fields.iter().enumerate() {
            self.declarations.insert(
                field.id.clone(),
                Declaration {
                    id: field.id.clone(),
                    name: field.name.clone(),
                    kind: DeclarationKind::Field,
                    identity_origin: field_origin(ordinal).ok_or_else(fail)?,
                    owner: Some(record.id.clone()),
                },
            );
        }
        Ok(())
    }
}
