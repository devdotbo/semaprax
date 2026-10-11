//! Independent source-signature and current namespace authentication for the
//! module-local primitive retained lexical replay context.
use super::*;
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
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
) -> Result<bool, Vec<Diagnostic>> {
    let Some(inventory) = module.static_text.as_deref() else {
        return Ok(false);
    };
    if inventory.lexical.is_none() {
        return Ok(false);
    }
    let mut physical =
        crate::cleanup_plan::ReplayPhysicalReservation::new(MAX_CHANGE_BUILDER_BYTES);
    physical
        .reserve_array::<Context<'_>>(1)
        .map_err(|_| fail())?;
    let context = authenticate(module, facts, &mut physical)?;
    let mut found = false;
    for function in &module.functions {
        if crate::hir::lexical_exit::requires(function).map_err(|error| vec![error])? {
            physical = crate::cleanup_plan::validate_retained_lexical_with_reservation(
                &context, function, physical,
            )
            .map_err(|error| {
                let errors = fail();
                // Keep the public retained-source diagnostic stable. Owning
                // tests must distinguish its independently replayed cause from
                // a literal-site/namespace mismatch before proposing a repair.
                #[cfg(test)]
                let errors = errors
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
    }
    if !found {
        return Err(fail());
    }
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
