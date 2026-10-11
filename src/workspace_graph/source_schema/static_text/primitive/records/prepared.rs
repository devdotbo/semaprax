//! Source-layout authentication and minimal index construction share the session
//! preparation. A current namespace fact is compared data, never body authority.
use super::*;
use crate::cleanup_plan::SelectedReplayPreparation;
fn debit(
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
    n: usize,
) -> Result<(), Vec<Diagnostic>> {
    resources
        .charge(anchor, n, "retained record source authentication")
        .map_err(|_| fail())
}
fn fact<'a>(
    id: &str,
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<&'a WorkspaceDeclarationFact, Vec<Diagnostic>> {
    for (key, value) in facts {
        debit(
            resources,
            anchor,
            key.len()
                .checked_add(id.len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(fail)?,
        )?;
        if key == id {
            return Ok(value);
        }
    }
    Err(fail())
}
fn origin(explicit: bool) -> hir::IdentityOrigin {
    if explicit {
        hir::IdentityOrigin::Explicit
    } else {
        hir::IdentityOrigin::Automatic
    }
}
pub(in crate::workspace_graph::source_schema::static_text::primitive) fn authenticate(
    module: &WorkspaceResolvedModule,
    source: Option<&Records>,
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    index: &mut hir::DeclarationIndex,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<(), Vec<Diagnostic>> {
    let answer = authenticate_inner(module, source, facts, index, resources, anchor, true, None);
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
) -> Result<(), Vec<Diagnostic>> {
    let result = authenticate_inner(
        module,
        Some(source),
        facts,
        index,
        resources,
        anchor,
        false,
        Some(demanded),
    );
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
) -> Result<(), Vec<Diagnostic>> {
    let expected = source.map_or(&[][..], |source| source.records.as_slice());
    debit(resources, anchor, 1)?;
    if module.types.len() != expected.len() {
        return Err(fail());
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
                    .ok_or_else(fail)?,
            )?;
            if current.id != *id {
                continue;
            }
        }
        selected = selected.checked_add(1).ok_or_else(fail)?;
        let hir::ResolvedTypeDeclarationKind::Record { fields } = &current.kind else {
            return Err(fail());
        };
        let hir::ResolvedTypeDeclarationKind::Record {
            fields: source_fields,
        } = &source.declaration.kind
        else {
            return Err(fail());
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
                .ok_or_else(fail)?,
        )?;
        if !current.type_parameters.is_empty()
            || !source.declaration.type_parameters.is_empty()
            || current.id != source.declaration.id
            || current.name != source.declaration.name
            || current.span != source.declaration.span
            || fields.len() != source_fields.len()
            || fields.len() != source.field_explicit.len()
        {
            return Err(fail());
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
                    .ok_or_else(fail)?,
            )?;
            // Flat scalar/String/Bytes types have no recursively allocated keys.
            if !(hir::is_scalar_resolved_type(&field.ty)
                || matches!(
                    field.ty,
                    hir::ResolvedType::String | hir::ResolvedType::Bytes
                ))
                || field != old
            {
                return Err(fail());
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
        resources
            .construct(anchor, |physical, debit| {
                index.append_flat_record_replay_metered(
                    current,
                    origin(source.explicit),
                    |i| source.field_explicit.get(i).copied().map(origin),
                    physical,
                    debit,
                )
            })
            .map_err(|_| fail())?;
    }
    if demanded.is_some() && selected != 1 {
        return Err(fail());
    }
    // Canonical builtin metadata is reconstructed once. It does not select an
    // element shape or turn generic instantiation into imported authority.
    if append_vec {
        resources
            .construct(anchor, |physical, debit| {
                index.append_vec_replay_metered(physical, debit)
            })
            .map_err(|_| fail())?;
    }
    Ok(())
}
fn namespace(
    fact: &WorkspaceDeclarationFact,
    module: &WorkspaceResolvedModule,
    kind: hir::DeclarationKind,
    owner: Option<&str>,
    origin: hir::IdentityOrigin,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<(), Vec<Diagnostic>> {
    let units = module
        .path
        .len()
        .checked_add(module.module.len())
        .and_then(|n| n.checked_add(fact.path.as_ref().map_or(0, String::len)))
        .and_then(|n| n.checked_add(fact.module.as_ref().map_or(0, String::len)))
        .and_then(|n| n.checked_add(fact.owner.as_ref().map_or(0, String::len)))
        .and_then(|n| n.checked_add(owner.map_or(0, str::len)))
        .and_then(|n| n.checked_add(1))
        .ok_or_else(fail)?;
    debit(resources, anchor, units)?;
    if fact.kind != kind
        || fact.origin != origin
        || fact.owner.as_deref() != owner
        || fact.path.as_deref() != Some(module.path.as_str())
        || fact.module.as_deref() != Some(module.module.as_str())
    {
        return Err(fail());
    }
    Ok(())
}
