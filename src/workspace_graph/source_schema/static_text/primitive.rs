//! Independent source-signature and current namespace authentication for the
//! module-local primitive retained lexical replay context.
use super::*;
mod helpers;
mod records;
use crate::cleanup::ReplayContext;
use crate::hir::{
    ResolvedFunction, ResolvedFunctionInstance, ResolvedFunctionTemplate, ResolvedInterface,
    ResolvedTypeDeclaration,
};

pub(super) struct Context<'a> {
    index: hir::DeclarationIndex,
    module: &'a WorkspaceResolvedModule,
}
impl ReplayContext for Context<'_> {
    fn declarations(&self) -> &hir::DeclarationIndex {
        &self.index
    }
    fn types(&self) -> &[ResolvedTypeDeclaration] {
        &self.module.types
    }
    fn interfaces(&self) -> &[ResolvedInterface] {
        &[]
    }
    fn functions(&self) -> &[ResolvedFunction] {
        &self.module.functions
    }
    fn function_templates(&self) -> &[ResolvedFunctionTemplate] {
        &[]
    }
    fn function_instances(&self) -> &[ResolvedFunctionInstance] {
        &[]
    }
}

#[cfg(test)]
pub(super) fn authenticate<'a>(
    module: &'a WorkspaceResolvedModule,
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    physical: &mut crate::cleanup_plan::ReplayPhysicalReservation,
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
    physical
        .reserve(crate::workspace_graph::static_text::primitive::digest_scratch_bytes())
        .map_err(|_| fail())?;
    let mut work = WORK;
    for (source, function) in inventory.functions.iter().zip(&module.functions) {
        let declaration = facts.get(source.id.as_str()).ok_or_else(fail)?;
        if declaration.kind != hir::DeclarationKind::Function
            || declaration.owner.is_some()
            || declaration.origin
                != if source.explicit {
                    hir::IdentityOrigin::Explicit
                } else {
                    hir::IdentityOrigin::Automatic
                }
            || declaration.path.as_deref() != Some(module.path.as_str())
            || declaration.module.as_deref() != Some(module.module.as_str())
        {
            return Err(fail());
        }
        if function.id != source.id
            || !crate::workspace_graph::static_text::primitive::body_matches(
                inventory, source, function,
            )
            || function.name != source.name
            || !function.effects.is_empty()
            || function.yields.is_some()
            || function.return_type != source.result
            || function.params.len() != source.params.len()
        {
            return Err(fail());
        }
        work = work.checked_sub(source.params.len()).ok_or_else(fail)?;
        for (source, parameter) in source.params.iter().zip(&function.params) {
            if parameter.name != source.name
                || parameter.ownership != source.ownership
                || parameter.ty != source.ty
            {
                return Err(fail());
            }
        }
    }
    // Exact authored declaration order is part of this rare private inventory.
    // Reconstruct from current typed functions only after the complete source
    // body guard and current namespace/signature comparison, with no searches.
    let origins = module
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
        });
    let mut index =
        hir::DeclarationIndex::primitive_replay(origins, physical).map_err(|e| vec![e])?;
    records::authenticate(
        module,
        inventory.records.as_deref(),
        facts,
        &mut index,
        physical,
    )?;
    Ok(Context { index, module })
}

/// Static literal sites have already been independently authenticated by the
/// caller. Rebuild primitive declarations, then run complete typed lexical
/// replay with the same cumulative reservation for every selected function.
pub(in crate::workspace_graph::source_schema) fn replay(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
) -> Result<bool, Vec<Diagnostic>> {
    let Some(inventory) = module.static_text.as_deref() else {
        return Ok(false);
    };
    if inventory.lexical.is_none() {
        return Ok(false);
    }
    let physical = crate::cleanup_plan::ReplayPhysicalReservation::new(MAX_CHANGE_BUILDER_BYTES);
    if inventory
        .lexical
        .as_deref()
        .is_some_and(|inventory| inventory.imported)
    {
        helpers::replay(module, modules, facts, physical)?;
        return Ok(true);
    }
    let anchor = module.functions.first().ok_or_else(fail)?;
    let mut preparation = crate::cleanup_plan::SelectedReplayPreparation::new(anchor, physical)
        .map_err(|_| fail())?;
    current_module(module, modules, &mut preparation, anchor)?;
    preparation
        .reserve_array::<Context<'_>>(anchor, 1, "retained lexical context")
        .map_err(|_| fail())?;
    let context = helpers::local::authenticate(module, facts, &mut preparation, anchor)?;
    preparation
        .reserve_array::<&ResolvedFunction>(
            anchor,
            module.functions.len(),
            "retained lexical member references",
        )
        .map_err(|_| fail())?;
    let mut lexical = Vec::with_capacity(module.functions.len());
    if lexical.capacity() != module.functions.len() {
        preparation.fail();
        return Err(fail());
    }
    for function in &module.functions {
        if helpers::local::requires(function, &mut preparation, anchor)? {
            preparation
                .charge(anchor, 1, "retained lexical member insertion")
                .map_err(|_| fail())?;
            lexical.push(function);
        }
    }
    let mut session =
        crate::cleanup_plan::SelectedReplaySession::from_preparation(&context, anchor, preparation)
            .map_err(|_| fail())?;
    let mut found = false;
    for function in lexical {
        session.validate(function).map_err(|error| {
            let errors = fail();
            // Keep the public retained-source diagnostic stable. Owning
            // tests must distinguish its independently replayed cause from
            // a literal-site/namespace mismatch before proposing a repair.
            #[cfg(test)]
            let errors: Vec<_> = errors
                .into_iter()
                .map(|diagnostic| {
                    let prefix = error.message.get(..error.message.len().min(512));
                    let detail = prefix.unwrap_or_else(|| {
                        let mut end = error.message.len().min(512);
                        while !error.message.is_char_boundary(end) {
                            end -= 1;
                        }
                        &error.message[..end]
                    });
                    diagnostic.with_help(format!(
                        "independent primitive lexical replay for `{}`: {}: {}",
                        function.id, error.code, detail
                    ))
                })
                .collect();
            #[cfg(not(test))]
            let _ = error;
            errors
        })?;
        found = true;
    }
    if !found {
        return Err(fail());
    }
    session.finish().map_err(|_| fail())?;
    Ok(true)
}

// Test callers acquire the real held-source context; no metadata is projected away.
#[cfg(test)]
pub(in crate::workspace_graph) fn with_test_context<T>(
    module: &WorkspaceResolvedModule,
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    apply: impl FnOnce(&dyn ReplayContext) -> T,
) -> Result<T, Vec<Diagnostic>> {
    let mut physical =
        crate::cleanup_plan::ReplayPhysicalReservation::new(MAX_CHANGE_BUILDER_BYTES);
    let context = authenticate(module, facts, &mut physical)?;
    Ok(apply(&context))
}

/// Tests hand in the sole owned preparation, and receive it back only with
/// the current source-authenticated context held for the callback's duration.
#[cfg(test)]
pub(in crate::workspace_graph) fn with_prepared_test_context<T>(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    mut preparation: crate::cleanup_plan::SelectedReplayPreparation,
    apply: impl FnOnce(&dyn ReplayContext, crate::cleanup_plan::SelectedReplayPreparation) -> T,
) -> Result<T, Vec<Diagnostic>> {
    let anchor = module.functions.first().ok_or_else(fail)?;
    current_module(module, modules, &mut preparation, anchor)?;
    let imported = module
        .static_text
        .as_ref()
        .and_then(|i| i.lexical.as_ref())
        .is_some_and(|i| i.imported);
    if imported {
        let context = helpers::authenticate(module, modules, facts, &mut preparation)?;
        return Ok(apply(&context, preparation));
    }
    preparation
        .reserve_array::<Context<'_>>(anchor, 1, "held test source context")
        .map_err(|_| fail())?;
    let context = helpers::local::authenticate(module, facts, &mut preparation, anchor)?;
    Ok(apply(&context, preparation))
}

fn current_module(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    preparation: &mut crate::cleanup_plan::SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<(), Vec<Diagnostic>> {
    let mut exact = 0usize;
    let mut namespace = 0usize;
    for current in modules {
        preparation
            .charge(
                anchor,
                current
                    .path
                    .len()
                    .checked_add(module.path.len())
                    .and_then(|n| n.checked_add(current.module.len()))
                    .and_then(|n| n.checked_add(module.module.len()))
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(fail)?,
                "retained current module membership",
            )
            .map_err(|_| fail())?;
        if std::ptr::eq(current, module) {
            exact += 1;
        }
        if current.path == module.path || current.module == module.module {
            namespace += 1;
        }
    }
    if exact != 1 || namespace != 1 {
        preparation.fail();
        return Err(fail());
    }
    Ok(())
}
