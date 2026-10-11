//! Local source authentication for the first imported scalar closure profile.
//! The no-import retained-record route keeps its existing independent owner.
use super::*;
fn primitive(ty: &hir::ResolvedType) -> bool {
    hir::is_scalar_resolved_type(ty)
        || matches!(ty, hir::ResolvedType::Nominal { declaration, arguments }
            if arguments.is_empty() || declaration.as_str() == crate::prelude::VEC_ID && arguments.len() == 1)
        || matches!(
            ty,
            hir::ResolvedType::ArrayU8(_)
                | hir::ResolvedType::String
                | hir::ResolvedType::Bytes
                | hir::ResolvedType::Str
                | hir::ResolvedType::SliceU8
        )
}
pub(in crate::workspace_graph::source_schema::static_text::primitive) fn authenticate<'a>(
    module: &'a WorkspaceResolvedModule,
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<Context<'a>, Vec<Diagnostic>> {
    let answer = authenticate_inner(module, facts, resources, anchor);
    if answer.is_err() {
        resources.fail();
    }
    answer
}
fn authenticate_inner<'a>(
    module: &'a WorkspaceResolvedModule,
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<Context<'a>, Vec<Diagnostic>> {
    let inventory = module
        .static_text
        .as_deref()
        .and_then(|i| i.lexical.as_deref())
        .ok_or_else(fail)?;
    if !module.interfaces.is_empty()
        || !module.permits.is_empty()
        || !module.agents.is_empty()
        || !module.function_templates.is_empty()
        || !module.function_instances.is_empty()
        || module.functions.len() != inventory.functions.len()
    {
        return Err(fail());
    }
    charge(resources, anchor, inventory.capture_hash_work)?;
    resources
        .reserve_array::<u8>(
            anchor,
            crate::workspace_graph::static_text::primitive::digest_scratch_bytes(),
            "imported consumer body digest scratch",
        )
        .map_err(|_| fail())?;
    let mut local_work = WORK;
    for (source, function) in inventory.functions.iter().zip(&module.functions) {
        let origin = fact(source.id.as_str(), facts, resources, anchor)?;
        let units = source
            .id
            .as_str()
            .len()
            .checked_add(function.id.as_str().len())
            .and_then(|n| n.checked_add(source.name.len()))
            .and_then(|n| n.checked_add(function.name.len()))
            .and_then(|n| n.checked_add(module.path.len()))
            .and_then(|n| n.checked_add(module.module.len()))
            .and_then(|n| n.checked_add(origin.path.as_ref().map_or(0, String::len)))
            .and_then(|n| n.checked_add(origin.module.as_ref().map_or(0, String::len)))
            .and_then(|n| n.checked_add(1))
            .ok_or_else(fail)?;
        charge(resources, anchor, units)?;
        debit_type(&source.result, resources, anchor)?;
        debit_type(&function.return_type, resources, anchor)?;
        if origin.kind != hir::DeclarationKind::Function
            || origin.owner.is_some()
            || origin.origin
                != if source.explicit {
                    hir::IdentityOrigin::Explicit
                } else {
                    hir::IdentityOrigin::Automatic
                }
            || origin.path.as_deref() != Some(module.path.as_str())
            || origin.module.as_deref() != Some(module.module.as_str())
            || function.id != source.id
            || function.name != source.name
            || !function.effects.is_empty()
            || function.yields.is_some()
            || !primitive(&function.return_type)
            || !primitive(&source.result)
            || function.return_type != source.result
            || function.params.len() != source.params.len()
        {
            return Err(fail());
        }
        local_work = local_work
            .checked_sub(source.params.len())
            .ok_or_else(fail)?;
        for (source, current) in source.params.iter().zip(&function.params) {
            debit_type(&source.ty, resources, anchor)?;
            debit_type(&current.ty, resources, anchor)?;
            charge(
                resources,
                anchor,
                source
                    .name
                    .len()
                    .checked_add(current.name.len())
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(fail)?,
            )?;
            if !primitive(&source.ty)
                || !primitive(&current.ty)
                || source.name != current.name
                || source.ownership != current.ownership
                || source.ty != current.ty
            {
                return Err(fail());
            }
        }
        if !crate::workspace_graph::static_text::primitive::body_matches_metered(
            inventory,
            source,
            function,
            &mut |units| charge(resources, anchor, units).ok(),
        ) {
            return Err(fail());
        }
    }
    let mut index = resources
        .construct(anchor, |physical, debit| {
            hir::DeclarationIndex::primitive_replay_metered(
                module
                    .functions
                    .iter()
                    .zip(&inventory.functions)
                    .map(|(function, source)| {
                        (
                            function,
                            if source.explicit {
                                hir::IdentityOrigin::Explicit
                            } else {
                                hir::IdentityOrigin::Automatic
                            },
                        )
                    }),
                physical,
                debit,
            )
        })
        .map_err(|_| fail())?;
    super::super::records::authenticate_prepared(
        module,
        inventory.records.as_deref(),
        facts,
        &mut index,
        resources,
        anchor,
    )?;
    Ok(Context { index, module })
}

/// Same borrowed lexical classifier, with each frontier transition debited.
pub(super) fn requires(
    function: &ResolvedFunction,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<bool, Vec<Diagnostic>> {
    type Cursor<'a> = Option<hir::lexical_exit::ChildCursor<'a>>;
    const LIMIT: usize = crate::cache_codec::MAX_DEPTH + 1;
    resources
        .reserve_array::<Cursor<'_>>(anchor, LIMIT, "imported consumer lexical classifier")
        .map_err(|_| fail())?;
    let mut stack = [None; LIMIT];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        stack[0] = Some(hir::lexical_exit::ChildCursor::new(root));
        let mut depth = 1;
        while depth != 0 {
            charge(resources, anchor, 1)?;
            let cursor = stack[depth - 1].as_mut().ok_or_else(fail)?;
            if !cursor.is_started()
                && matches!(
                    &cursor.expression().kind,
                    hir::ResolvedExprKind::LexicalExit { .. }
                )
            {
                return Ok(true);
            }
            if let Some(child) = cursor.next() {
                if depth == LIMIT {
                    return Err(fail());
                }
                stack[depth] = Some(hir::lexical_exit::ChildCursor::new(child));
                depth += 1;
            } else {
                stack[depth - 1] = None;
                depth -= 1;
            }
        }
    }
    Ok(false)
}

fn debit_type(
    ty: &hir::ResolvedType,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<(), Vec<Diagnostic>> {
    charge(resources, anchor, 1)?;
    if let hir::ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    {
        charge(resources, anchor, declaration.as_str().len())?;
        if declaration.as_str() == crate::prelude::VEC_ID {
            let [element] = arguments.as_slice() else {
                return Err(fail());
            };
            if matches!(element, hir::ResolvedType::Nominal { arguments, .. } if !arguments.is_empty())
            {
                return Err(fail());
            }
            debit_type(element, resources, anchor)?;
        } else if !arguments.is_empty() {
            return Err(fail());
        }
    }
    Ok(())
}
