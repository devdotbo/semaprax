//! Source-layout authentication and minimal index construction share the session
//! preparation. A current namespace fact is compared data, never body authority.
use super::*;
use crate::cleanup_plan::SelectedReplayPreparation;
fn debit(
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
    n: usize,
) -> Result<(), Refused> {
    resources.try_charge(anchor, n, "retained record source authentication")
}
fn fact<'a>(
    id: &str,
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<&'a WorkspaceDeclarationFact, Refused> {
    for (key, value) in facts {
        debit(
            resources,
            anchor,
            key.len()
                .checked_add(id.len())
                .and_then(|n| n.checked_add(1))
                .ok_or(Refused::MISMATCH)?,
        )?;
        if key == id {
            return Ok(value);
        }
    }
    Err(Refused::MISMATCH)
}
fn origin(explicit: bool) -> hir::IdentityOrigin {
    if explicit {
        hir::IdentityOrigin::Explicit
    } else {
        hir::IdentityOrigin::Automatic
    }
}
/// Returns the canonical Vec current type the receiving directory lends beside
/// the module's own declarations; the index holds its matching metadata.
pub(in crate::workspace_graph::source_schema::static_text::primitive) fn authenticate(
    module: &WorkspaceResolvedModule,
    source: Option<&Records>,
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    index: &mut hir::DeclarationIndex,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<hir::ResolvedTypeDeclaration, Refused> {
    let answer = authenticate_inner(module, source, facts, index, resources, anchor, true, None)
        .and_then(|vector| vector.ok_or(Refused::MISMATCH));
    if answer.is_err() {
        resources.fail();
    }
    answer
}
pub(in crate::workspace_graph::source_schema::static_text::primitive) fn authenticate_provider(
    module: &WorkspaceResolvedModule,
    source: &Records,
    demanded: &hir::DeclarationId,
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    index: &mut hir::DeclarationIndex,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<(), Refused> {
    let result = authenticate_inner(
        module,
        Some(source),
        facts,
        index,
        resources,
        anchor,
        false,
        Some(demanded),
    )
    .and_then(|vector| match vector {
        None => Ok(()),
        Some(_) => Err(Refused::MISMATCH),
    });
    if result.is_err() {
        resources.fail();
    }
    result
}
#[allow(
    clippy::too_many_arguments,
    reason = "the provider preserves the independently authenticated local Vec index"
)]
fn authenticate_inner(
    module: &WorkspaceResolvedModule,
    source: Option<&Records>,
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    index: &mut hir::DeclarationIndex,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
    append_vec: bool,
    demanded: Option<&hir::DeclarationId>,
) -> Result<Option<hir::ResolvedTypeDeclaration>, Refused> {
    let expected = source.map_or(&[][..], |source| source.records.as_slice());
    debit(resources, anchor, 1)?;
    if module.types.len() != expected.len() {
        return Err(Refused::MISMATCH);
    }
    let mut selected = 0usize;
    for (source, current) in expected.iter().zip(&module.types) {
        if let Some(id) = demanded {
            debit(
                resources,
                anchor,
                id.as_str()
                    .len()
                    .checked_add(current.id.as_str().len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Refused::MISMATCH)?,
            )?;
            if current.id != *id {
                continue;
            }
        }
        selected = selected.checked_add(1).ok_or(Refused::MISMATCH)?;
        let hir::ResolvedTypeDeclarationKind::Record { fields } = &current.kind else {
            return Err(Refused::MISMATCH);
        };
        let hir::ResolvedTypeDeclarationKind::Record {
            fields: source_fields,
        } = &source.declaration.kind
        else {
            return Err(Refused::MISMATCH);
        };
        debit(
            resources,
            anchor,
            current
                .id
                .as_str()
                .len()
                .checked_add(source.declaration.id.as_str().len())
                .and_then(|n| n.checked_add(current.name.len()))
                .and_then(|n| n.checked_add(source.declaration.name.len()))
                .and_then(|n| n.checked_add(1))
                .ok_or(Refused::MISMATCH)?,
        )?;
        if !current.type_parameters.is_empty()
            || !source.declaration.type_parameters.is_empty()
            || current.id != source.declaration.id
            || current.name != source.declaration.name
            || current.span != source.declaration.span
            || fields.len() != source_fields.len()
            || fields.len() != source.field_explicit.len()
        {
            return Err(Refused::MISMATCH);
        }
        for (field, old) in fields.iter().zip(source_fields) {
            debit(
                resources,
                anchor,
                field
                    .id
                    .as_str()
                    .len()
                    .checked_add(old.id.as_str().len())
                    .and_then(|n| n.checked_add(field.name.len()))
                    .and_then(|n| n.checked_add(old.name.len()))
                    .and_then(|n| n.checked_add(1))
                    .ok_or(Refused::MISMATCH)?,
            )?;
            // Flat scalar/String/Bytes types have no recursively allocated keys.
            if !(hir::is_scalar_resolved_type(&field.ty)
                || matches!(
                    field.ty,
                    hir::ResolvedType::String | hir::ResolvedType::Bytes
                ))
                || field != old
            {
                return Err(Refused::MISMATCH);
            }
        }
        let declaration = fact(current.id.as_str(), facts, resources, anchor)?;
        namespace(
            declaration,
            module,
            hir::DeclarationKind::Record,
            None,
            origin(source.explicit),
            resources,
            anchor,
        )?;
        for (field, explicit) in fields.iter().zip(&source.field_explicit) {
            let declaration = fact(field.id.as_str(), facts, resources, anchor)?;
            namespace(
                declaration,
                module,
                hir::DeclarationKind::Field,
                Some(current.id.as_str()),
                origin(*explicit),
                resources,
                anchor,
            )?;
        }
        resources.construct(anchor, |physical, debit| {
            index.append_flat_record_replay_metered(
                current,
                origin(source.explicit),
                |i| source.field_explicit.get(i).copied().map(origin),
                physical,
                debit,
            )
        })?;
    }
    if demanded.is_some() && selected != 1 {
        return Err(Refused::MISMATCH);
    }
    // Canonical builtin metadata is reconstructed once. It does not select an
    // element shape or turn generic instantiation into imported authority.
    if !append_vec {
        return Ok(None);
    }
    resources.construct(anchor, |physical, debit| {
        index.append_vec_replay_metered(physical, debit)
    })?;
    vector(resources, anchor).map(Some)
}
/// The receiving Vec classifier compares a current type declaration against
/// the index metadata above. Rebuild it from the same fixed compiler-owned
/// constants, never by copying the index it is compared with. The identity,
/// name, one exact parameter slot and its name are reserved before construction.
fn vector(
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<hir::ResolvedTypeDeclaration, Refused> {
    resources.construct(anchor, |physical, debit| {
        let refused = Refused::constructor("selected Vec type rebuild refused");
        let id = crate::prelude::VEC_ID;
        let names = id
            .len()
            .checked_add("Vec".len())
            .and_then(|n| n.checked_add("T".len()))
            .ok_or(refused)?;
        debit(names.checked_add(1).ok_or(refused)?)?;
        let bytes = names
            .checked_add(std::mem::size_of::<hir::ResolvedTypeParameterDeclaration>())
            .ok_or(refused)?;
        physical.reserve(bytes).map_err(|_| refused)?;
        let mut type_parameters = Vec::with_capacity(1);
        if type_parameters.capacity() != 1 {
            return Err(refused);
        }
        type_parameters.push(hir::ResolvedTypeParameterDeclaration {
            name: "T".into(),
            index: 0,
            span: crate::ast::Span::default(),
        });
        Ok(hir::ResolvedTypeDeclaration {
            id: hir::DeclarationId::new(id),
            name: "Vec".into(),
            type_parameters,
            kind: hir::ResolvedTypeDeclarationKind::Record { fields: Vec::new() },
            span: crate::ast::Span::default(),
        })
    })
}
fn namespace(
    fact: &WorkspaceDeclarationFact,
    module: &WorkspaceResolvedModule,
    kind: hir::DeclarationKind,
    owner: Option<&str>,
    origin: hir::IdentityOrigin,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<(), Refused> {
    let units = module
        .path
        .len()
        .checked_add(module.module.len())
        .and_then(|n| n.checked_add(fact.path.as_ref().map_or(0, String::len)))
        .and_then(|n| n.checked_add(fact.module.as_ref().map_or(0, String::len)))
        .and_then(|n| n.checked_add(fact.owner.as_ref().map_or(0, String::len)))
        .and_then(|n| n.checked_add(owner.map_or(0, str::len)))
        .and_then(|n| n.checked_add(1))
        .ok_or(Refused::MISMATCH)?;
    debit(resources, anchor, units)?;
    if fact.kind != kind
        || fact.origin != origin
        || fact.owner.as_deref() != owner
        || fact.path.as_deref() != Some(module.path.as_str())
        || fact.module.as_deref() != Some(module.module.as_str())
    {
        return Err(Refused::MISMATCH);
    }
    Ok(())
}
