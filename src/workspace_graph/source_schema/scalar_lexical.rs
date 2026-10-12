//! Exact-source receiver for the closed ordinary scalar lexical family.
//!
//! This is a finite receiver profile beside the static-text primitive route,
//! accepting only the separate scalar lexical inventory. The actual caller's
//! fixed outward form is acquired on one existing paid owner before setup;
//! preparation, authentication, the Session constructor, every member replay
//! and finish then share that same affine Acquired -> Preparation -> Session
//! owner with no reset, refund or fresh allowance. Authentication refuses
//! with the Copy `SelectedReplayRefused`, never a Vec or String.
//!
//! Source binding comes from the receiving caller's own current authoritative
//! sources: the revision is recomputed from those bytes here, never copied
//! from the inventory, and every ordered member's typed digest must replay
//! under it. The module object must be an exact member of the live module
//! slice. Every lexical member is replayed by the paid lexical validator and
//! every other member by the paid scalar validator, on one Session.
//!
//! Funding limit: setup and authentication refusals are paid. A member replay
//! semantic refusal may still carry an unmigrated `EngineError::Owned`
//! Diagnostic built by the engine; that remains the known open obligation of
//! the selected replay protocol, so this route is not end-to-end funded.
use super::*;
mod received;
pub(super) use received::Received;
use crate::cleanup::ReplayContext;
use crate::cleanup_plan::{
    SelectedReplayPreparation, SelectedReplayRefusal, SelectedReplayRefused as Refused,
    SelectedReplayReport, SelectedReplayReportForm,
};
use crate::hir::{
    ResolvedFunction, ResolvedFunctionInstance, ResolvedFunctionTemplate, ResolvedInterface,
    ResolvedTypeDeclaration,
};
use crate::workspace_graph::static_text::scalar_lexical::{self as capture, census};

/// Held replay directory: the exact live module's own borrowed (possibly
/// sparse) members plus one declaration index rebuilt on the same preparation.
struct Context<'a> {
    index: hir::DeclarationIndex,
    module: &'a WorkspaceResolvedModule,
}
impl ReplayContext for Context<'_> {
    fn declarations(&self) -> &hir::DeclarationIndex {
        &self.index
    }
    fn types(&self) -> crate::hir::items::View<'_, ResolvedTypeDeclaration> {
        crate::hir::items::ItemSource::view(&self.module.types)
    }
    fn interfaces(&self) -> &[ResolvedInterface] {
        &self.module.interfaces
    }
    fn functions(&self) -> crate::hir::items::View<'_, ResolvedFunction> {
        crate::hir::items::ItemSource::view(&self.module.functions)
    }
    // The retained template carrier has no contiguous slice. Authentication
    // has already required this exact module's template set to be empty.
    fn function_templates(&self) -> &[ResolvedFunctionTemplate] {
        &[]
    }
    fn function_instances(&self) -> &[ResolvedFunctionInstance] {
        &self.module.function_instances
    }
}

/// `Ok(None)`: this module holds no scalar lexical inventory. Otherwise the
/// receipt is returned only after a successful paid finish.
pub(super) fn receive_paid<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    sources: &dyn ReceivingSources,
    form: SelectedReplayReportForm,
) -> Result<Option<Received<'a>>, SelectedReplayRefusal> {
    let Some(inventory) = module.scalar_lexical.as_deref() else {
        return Ok(None);
    };
    let physical = crate::cleanup_plan::ReplayPhysicalReservation::new(MAX_CHANGE_BUILDER_BYTES);
    let acquired = SelectedReplayReport::try_acquire(physical, form)?;
    let Some(anchor) = module.functions.first() else {
        return Err(acquired.refuse_unprepared());
    };
    let mut preparation = acquired.prepare(anchor)?;
    let (context, selected, flags) = match authenticate(
        module,
        modules,
        facts,
        sources,
        inventory,
        &mut preparation,
        anchor,
    ) {
        Ok(held) => held,
        Err(refused) => return Err(preparation.refuse(refused)),
    };
    let mut session = preparation.into_paid_session(&context, anchor)?;
    for (function, lexical) in module.functions.iter().zip(&selected) {
        // The first member refusal stops the route; no failed Session is
        // validated or finished again.
        let replayed = if *lexical {
            session.validate_paid(function)
        } else {
            session.validate_scalar_paid(function)
        };
        if let Err(error) = replayed {
            return Err(session.into_failed_preparation().refuse_engine(error));
        }
    }
    // Success drops the unused carrier and returns the same ledger, with no
    // refund or reset; it is released before the receipt is sealed.
    drop(session.finish_paid()?);
    Ok(Some(Received::seal(module, flags)))
}

/// Every step charges or reserves on the held preparation before it reads,
/// hashes or allocates. Refusal is the inline Copy reason only.
fn authenticate<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    sources: &dyn ReceivingSources,
    inventory: &capture::Inventory,
    preparation: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<(Context<'a>, Vec<bool>, census::Flags), Refused> {
    super::static_text::current_module(module, modules, preparation, anchor)?;
    let count = module.functions.len();
    preparation.try_charge(
        anchor,
        module
            .path
            .len()
            .checked_add(module.module.len())
            .and_then(|n| n.checked_mul(2))
            .and_then(|n| n.checked_add(1))
            .ok_or(Refused::MISMATCH)?,
        "scalar lexical inventory binding",
    )?;
    if !inventory.binds(&module.path, &module.module)
        || inventory.members().len() != count
        || !module.interfaces.is_empty()
        || !module.permits.is_empty()
        || !module.agents.is_empty()
        || !module.types.is_empty()
        || !module.function_templates.is_empty()
        || !module.function_instances.is_empty()
        || module.static_text.is_some()
        || module.record_closure.is_some()
        || module.native_law
    {
        return Err(Refused::MISMATCH);
    }
    let source = held_source(module, sources, preparation, anchor)?;
    preparation.try_reserve_array::<u8>(
        anchor,
        capture::digest_scratch_bytes(),
        "scalar lexical source digest scratch",
    )?;
    preparation.try_charge(
        anchor,
        capture::revision_units(&module.path, &module.module, source).ok_or(Refused::MISMATCH)?,
        "scalar lexical current source revision",
    )?;
    let revision = capture::revision(&module.path, &module.module, source);
    preparation.try_reserve_array::<bool>(anchor, count, "scalar lexical member census")?;
    let mut lexical = Vec::with_capacity(count);
    if lexical.capacity() != count {
        // The allocator's actual excess already exists: debit it on the same
        // ledger after the fact (never claimed prepaid). A refused debit is
        // the sticky first refusal; otherwise the mismatch poisons the owner.
        if let Some(excess) = lexical.capacity().checked_sub(count) {
            preparation.try_reserve_array::<bool>(anchor, excess, "scalar lexical member census")?;
        }
        preparation.fail();
        return Err(Refused::MISMATCH);
    }
    // One fixed census cursor backing serves every member walk; each walk
    // charges its own slot initialization before the array exists.
    preparation.try_reserve_array::<u8>(
        anchor,
        census::cursor_bytes(),
        "scalar lexical census cursor",
    )?;
    let mut flags = census::Flags::default();
    for (member, function) in inventory.members().iter().zip(&module.functions) {
        namespace(member, function, module, facts, preparation, anchor)?;
        if !capture::member_matches_metered(member, function, &revision, &mut |units| {
            preparation
                .try_charge(anchor, units, "scalar lexical member digest")
                .ok()
        }) {
            preparation.fail();
            return Err(Refused::MISMATCH);
        }
        let Some(observed) = census::function(function, &mut |units| {
            preparation.try_charge(anchor, units, "scalar lexical family census")
        })?
        else {
            preparation.fail();
            return Err(Refused::MISMATCH);
        };
        preparation.try_charge(anchor, 1, "scalar lexical member insertion")?;
        lexical.push(observed.lexical);
        flags.merge(observed);
    }
    if !flags.lexical {
        preparation.fail();
        return Err(Refused::MISMATCH);
    }
    preparation.try_reserve_array::<Context<'_>>(anchor, 1, "scalar lexical replay context")?;
    let index = preparation.construct(anchor, |physical, debit| {
        hir::DeclarationIndex::primitive_replay_metered(
            module
                .functions
                .iter()
                .zip(inventory.members())
                .map(|(function, member)| (function, member.origin())),
            physical,
            debit,
        )
    })?;
    Ok((Context { index, module }, lexical, flags))
}

/// Exactly one current source for this module's path, from the caller.
fn held_source<'s>(
    module: &WorkspaceResolvedModule,
    sources: &'s dyn ReceivingSources,
    preparation: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<&'s str, Refused> {
    let mut held = None;
    for index in 0..sources.source_count() {
        let (path, text) = sources.source_at(index).ok_or(Refused::MISMATCH)?;
        preparation.try_charge(
            anchor,
            path.len()
                .checked_add(module.path.len())
                .and_then(|n| n.checked_add(1))
                .ok_or(Refused::MISMATCH)?,
            "scalar lexical source selection",
        )?;
        if path == module.path && held.replace(text).is_some() {
            return Err(Refused::MISMATCH);
        }
    }
    held.ok_or(Refused::MISMATCH)
}

/// Current namespace fact for one ordered member: a top-level function of
/// this exact path and module with the captured identity origin.
fn namespace(
    member: &capture::Member,
    function: &ResolvedFunction,
    module: &WorkspaceResolvedModule,
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    preparation: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<(), Refused> {
    let id = member.id().as_str();
    let mut found = None;
    for (key, value) in facts {
        preparation.try_charge(
            anchor,
            key.len()
                .checked_add(id.len())
                .and_then(|n| n.checked_add(1))
                .ok_or(Refused::MISMATCH)?,
            "scalar lexical namespace lookup",
        )?;
        if key == id {
            found = Some(value);
            break;
        }
    }
    let fact = found.ok_or(Refused::MISMATCH)?;
    preparation.try_charge(
        anchor,
        module
            .path
            .len()
            .checked_add(module.module.len())
            .and_then(|n| n.checked_add(fact.path.as_ref().map_or(0, String::len)))
            .and_then(|n| n.checked_add(fact.module.as_ref().map_or(0, String::len)))
            .and_then(|n| n.checked_add(function.id.as_str().len()))
            .and_then(|n| n.checked_add(id.len()))
            .and_then(|n| n.checked_add(1))
            .ok_or(Refused::MISMATCH)?,
        "scalar lexical namespace comparison",
    )?;
    if fact.kind != hir::DeclarationKind::Function
        || fact.owner.is_some()
        || fact.origin != member.origin()
        || fact.path.as_deref() != Some(module.path.as_str())
        || fact.module.as_deref() != Some(module.module.as_str())
        || function.id != *member.id()
    {
        return Err(Refused::MISMATCH);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
