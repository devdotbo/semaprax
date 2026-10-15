//! Independently paid class-inheritance producer; the complete old helper stays.
use super::identity_payer::IdentityIndexError;
use super::nominal_paid_values as paid;
use super::nominal_rollback::PendingVec;
use super::*;
use std::mem::size_of;
impl DeclarationIndex {
    pub(super) fn materialize_class_inheritance_paid<E>(
        &mut self,
        program: &dyn ProgramSource,
        reserve: &mut paid::Reserve<'_, E>,
    ) -> paid::Result<(), E> {
        paid::bytes(reserve, 0)?;
        for declaration in program.types() {
            let Some(extends) = &declaration.extends else {
                continue;
            };
            let child = self.inserted_identity(&declaration.stable_id);
            let Type::Named {
                name: parent_name,
                arguments: parent_arguments,
            } = extends
            else {
                return Err(paid::diagnostic(
                    "SPX-T227",
                    format_args!(
                        "class `{}` must extend a named class type",
                        declaration.name
                    ),
                    declaration.name_span,
                    program.path(),
                    reserve,
                )?
                .into());
            };
            if !parent_arguments.is_empty() {
                return Err(paid::diagnostic("SPX-T227", format_args!("class `{}` extends generic type `{parent_name}`; inheritance over generic classes is closed in this slice", declaration.name), declaration.name_span, program.path(), reserve)?.into());
            }
            let Some(parent) = self.types_by_name.get(parent_name) else {
                return Err(paid::diagnostic(
                    "SPX-T227",
                    format_args!(
                        "class `{}` extends unknown type `{parent_name}`",
                        declaration.name
                    ),
                    declaration.name_span,
                    program.path(),
                    reserve,
                )?
                .into());
            };
            if self
                .declaration(parent)
                .is_none_or(|item| item.kind != DeclarationKind::Class)
            {
                return Err(paid::diagnostic(
                    "SPX-T227",
                    format_args!(
                        "class `{}` extends non-class type `{parent_name}`",
                        declaration.name
                    ),
                    declaration.name_span,
                    program.path(),
                    reserve,
                )?
                .into());
            }
            if !self.class_parents.contains_key(&child) {
                paid::node::<DeclarationId, DeclarationId, E>(reserve)?;
            }
            self.class_parents.insert(child, parent.clone());
        }
        for declaration in program.types() {
            if !matches!(declaration.kind, TypeDeclarationKind::Class { .. }) {
                continue;
            }
            let child = self.inserted_identity(&declaration.stable_id);
            paid::bytes(reserve, size_of::<BTreeSet<DeclarationId>>())?;
            let mut visited = BTreeSet::new();
            let mut cursor = child.clone();
            while let Some(parent) = self.class_parents.get(&cursor) {
                if parent == &child || visited.contains(parent) {
                    return Err(paid::diagnostic(
                        "SPX-T228",
                        format_args!(
                            "class `{}` participates in an inheritance cycle",
                            declaration.name
                        ),
                        declaration.span,
                        program.path(),
                        reserve,
                    )?
                    .into());
                }
                paid::node::<DeclarationId, (), E>(reserve)?;
                visited.insert(parent.clone());
                cursor = parent.clone();
            }
        }
        paid::bytes(
            reserve,
            size_of::<BTreeMap<DeclarationId, PendingVec<ResolvedFieldDeclaration>>>(),
        )?;
        let mut declared_fields = BTreeMap::new();
        for declaration in program.types() {
            if !matches!(declaration.kind, TypeDeclarationKind::Class { .. }) {
                continue;
            }
            let id = self.inserted_identity(&declaration.stable_id);
            if let Some(fields) = self.record_fields(&id) {
                paid::node::<DeclarationId, PendingVec<ResolvedFieldDeclaration>, E>(reserve)?;
                paid::vector_fee::<ResolvedFieldDeclaration, E>(fields.len(), reserve)?;
                paid::bytes(
                    reserve,
                    size_of::<PendingVec<ResolvedFieldDeclaration>>()
                        + size_of::<
                            std::result::Result<
                                PendingVec<ResolvedFieldDeclaration>,
                                IdentityIndexError<E>,
                            >,
                        >(),
                )?;
                let mut snapshot = self.nominal_drop_workspace.fields(fields.len());
                for field in fields {
                    snapshot.push(paid::field(field, &self.nominal_drop_workspace, reserve)?);
                }
                declared_fields.insert(id, snapshot);
            }
        }
        for declaration in program.types() {
            let TypeDeclarationKind::Class { .. } = &declaration.kind else {
                continue;
            };
            let child = self.inserted_identity(&declaration.stable_id);
            let mut count = 1usize;
            let mut cursor = &child;
            while let Some(parent) = self.class_parents.get(cursor) {
                count = count.checked_add(1).ok_or_else(paid::overflow)?;
                cursor = parent;
            }
            let mut chain = paid::vector(count, reserve)?;
            let mut cursor = &child;
            while let Some(parent) = self.class_parents.get(cursor) {
                chain.push(parent.clone());
                cursor = parent;
            }
            chain.reverse();
            chain.push(child.clone());
            let mut capacity = 0usize;
            for member in &chain {
                if let Some(fields) = declared_fields.get(member) {
                    capacity = capacity
                        .checked_add(fields.len())
                        .ok_or_else(paid::overflow)?;
                }
            }
            paid::vector_fee::<ResolvedFieldDeclaration, E>(capacity, reserve)?;
            paid::bytes(
                reserve,
                size_of::<PendingVec<ResolvedFieldDeclaration>>()
                    + size_of::<BTreeSet<String>>()
                    + size_of::<BTreeSet<DeclarationId>>(),
            )?;
            let mut effective = self.nominal_drop_workspace.fields(capacity);
            let mut seen_names = BTreeSet::new();
            let mut seen_ids = BTreeSet::new();
            for member in &chain {
                let is_child = member == &child;
                let Some(fields) = declared_fields.get(member) else {
                    return Err(paid::diagnostic(
                        "SPX-H006",
                        format_args!("class `{member}` has no resolved fields"),
                        declaration.span,
                        program.path(),
                        reserve,
                    )?
                    .into());
                };
                for original in fields.iter() {
                    if seen_names.contains(&original.name) || seen_ids.contains(&original.id) {
                        return Err(paid::diagnostic(
                            "SPX-T229",
                            format_args!(
                                "class `{}` redeclares member `{}` from an ancestor",
                                declaration.name, original.name
                            ),
                            original.span,
                            program.path(),
                            reserve,
                        )?
                        .into());
                    }
                    let name = paid::text(&original.name, reserve)?;
                    paid::node::<String, (), E>(reserve)?;
                    seen_names.insert(name);
                    paid::node::<DeclarationId, (), E>(reserve)?;
                    seen_ids.insert(original.id.clone());
                    let Ok(index) = u32::try_from(effective.len()) else {
                        return Err(paid::diagnostic(
                            "SPX-H006",
                            format_args!("class `{}` has too many fields", declaration.name),
                            declaration.span,
                            program.path(),
                            reserve,
                        )?
                        .into());
                    };
                    if !is_child {
                        let key = (child.clone(), paid::text(&original.name, reserve)?);
                        if !self.fields_by_owner_name.contains_key(&key) {
                            paid::node::<(DeclarationId, String), DeclarationId, E>(reserve)?;
                        }
                        self.fields_by_owner_name.insert(key, original.id.clone());
                    }
                    let mut field = paid::field(original, &self.nominal_drop_workspace, reserve)?;
                    field.index = index;
                    effective.push(field);
                }
            }
            paid::bytes(
                reserve,
                size_of::<Option<NominalPayload<ResolvedFieldDeclaration>>>(),
            )?;
            if let Some(previous) = self.record_fields.insert(child, effective.into_vec()) {
                previous.drain_recursive_types(|ty| {
                    drop(self.nominal_drop_workspace.resolved_type(ty))
                });
            }
        }
        Ok(())
    }
}
