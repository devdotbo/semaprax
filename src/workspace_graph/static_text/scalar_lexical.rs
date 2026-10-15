//! Exact-source inventory for the closed ordinary scalar lexical family,
//! captured while the held authoritative source and the complete resolved
//! module are both present. It is a distinct receiver profile, not a broader
//! static-text inventory, and it grants nothing by itself: it retains no
//! source revision, plan or schema token. Reception recomputes the revision
//! from its own current authoritative source and replays every member before
//! any Graph family can be selected. Unsupported shapes return `None` and keep
//! their existing route and refusal.
use super::*;
use primitive::body_digest;
pub(in crate::workspace_graph) mod census;

const SOURCE_DOMAIN: usize = b"semaprax.retained-primitive-source.v1\0".len();

pub(in crate::workspace_graph) struct Inventory {
    module: String,
    path: String,
    members: Vec<Member>,
}
/// One authored retained member, in exact retained order. The digest covers
/// its span, identity, name, result identity and type, every parameter
/// identity/type/ownership, and every requires/body/ensures node, bound to
/// the capture-time source revision (which is then discarded).
pub(in crate::workspace_graph) struct Member {
    id: hir::DeclarationId,
    explicit: bool,
    digest: [u8; 32],
}
impl Inventory {
    pub(in crate::workspace_graph) fn binds(&self, path: &str, module: &str) -> bool {
        self.path == path && self.module == module
    }
    pub(in crate::workspace_graph) fn members(&self) -> &[Member] {
        &self.members
    }
    #[cfg(test)]
    pub(in crate::workspace_graph) fn members_mut(&mut self) -> &mut Vec<Member> {
        &mut self.members
    }
}
impl Member {
    pub(in crate::workspace_graph) fn id(&self) -> &hir::DeclarationId {
        &self.id
    }
    pub(in crate::workspace_graph) fn origin(&self) -> hir::IdentityOrigin {
        if self.explicit {
            hir::IdentityOrigin::Explicit
        } else {
            hir::IdentityOrigin::Automatic
        }
    }
}

/// The existing primitive source revision, over exact path, module and bytes.
pub(in crate::workspace_graph) fn revision(path: &str, module: &str, source: &str) -> [u8; 32] {
    body_digest::source(path, module, source)
}
/// Hash work of one `revision`, charged before it runs.
pub(in crate::workspace_graph) fn revision_units(
    path: &str,
    module: &str,
    source: &str,
) -> Option<usize> {
    source
        .len()
        .checked_add(path.len())?
        .checked_add(module.len())?
        .checked_add(24 + SOURCE_DOMAIN)
}
pub(in crate::workspace_graph) fn digest_scratch_bytes() -> usize {
    body_digest::scratch_bytes()
}
/// Exact identity plus the typed member digest under a caller-supplied
/// revision. The caller derives that revision from its own current source.
/// The identity comparison is debited before it runs.
pub(in crate::workspace_graph) fn member_matches_metered(
    member: &Member,
    function: &hir::ResolvedFunction,
    revision: &[u8; 32],
    debit: &mut dyn FnMut(usize) -> Option<()>,
) -> bool {
    key_units(member.id.as_str(), function.id.as_str()).is_some_and(|units| debit(units).is_some())
        && member.id == function.id
        && body_digest::typed_attempt_metered(function, revision, Some(debit)).0
            == Some(member.digest)
}

/// One string key comparison, priced as `function_matching::compare`.
fn key_units(left: &str, right: &str) -> Option<usize> {
    left.len().min(right.len()).checked_add(1)
}

/// A capacity mismatch after allocation. The allocator's actual excess
/// already exists: debit it after the fact (never claimed prepaid), then
/// poison the active owner so the first refusal stays sticky even when that
/// debit was admitted.
pub(in crate::workspace_graph) fn refuse_excess<T>(actual: usize, count: usize) -> Vec<Diagnostic> {
    if let Some(excess) = actual
        .checked_sub(count)
        .and_then(|slots| slots.checked_mul(std::mem::size_of::<T>()))
    {
        let _ = crate::bounded_output::reserve_active_required(excess);
    }
    let _ = crate::bounded_output::reserve_active_required(usize::MAX);
    capacity()
}

fn debit(work: &mut usize, units: usize) -> Result<(), Vec<Diagnostic>> {
    *work = work.checked_sub(units).ok_or_else(fail)?;
    Ok(())
}
/// Selection debit before demand is known: exhaustion declines the profile
/// (`Ok(None)`, existing route) instead of failing an unaffected module.
fn select(work: &mut usize, units: usize) -> Result<(), ()> {
    *work = work.checked_sub(units).ok_or(())?;
    Ok(())
}

/// Called by both retention routes before any output collection is filtered
/// or moved. Membership is the exact pairwise authored sequence, on which the
/// retention predicate holds for every member, so the retained function
/// sequence is exactly the captured member sequence.
pub(in crate::workspace_graph) fn capture(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    sources: Option<&[WorkspaceSource]>,
) -> Result<Option<Box<Inventory>>, Vec<Diagnostic>> {
    let Some(sources) = sources else {
        return Ok(None);
    };
    // No-import, declaration-closed modules only; every other shape keeps its
    // existing route. These are O(1) tests and charge no work or builder bytes.
    if !program.module_uses.is_empty()
        || !program.agents.is_empty()
        || !program.interfaces.is_empty()
        || !program.protocols.is_empty()
        || !program.implementations.is_empty()
        || !program.permits.is_empty()
        || !program.types.is_empty()
        || program.functions.is_empty()
        || program.functions.len() != resolved.functions.len()
        || !resolved.interfaces.is_empty()
        || !resolved.permits.is_empty()
        || !resolved.agents.is_empty()
        || !resolved.function_instances.is_empty()
    {
        return Ok(None);
    }
    let mut work = WORK;
    capture_metered(program, resolved, authored, sources, &mut work)
}

/// The metered capture after the O(1) shape gate, on the caller's work.
///
/// Selection runs in two allocation-free phases, both debited before they
/// read: (1) per member, the AST selector fields, one exact identity key
/// comparison and the census signature; (2) one reserved census cursor, then
/// each member's body walk with its own slot initialization. Membership is
/// pairwise: the resolver emits one function per non-generic authored AST
/// function in AST order, so with no imports, types or generics the authored
/// retention predicate holds exactly when every pair has the same identity.
/// No `BTreeMap` lookup (whose comparisons cannot be metered) is performed.
/// Any selection decline or exhaustion is `Ok(None)` with no builder bytes
/// beyond the cursor once phase (2) is reached.
pub(in crate::workspace_graph) fn capture_metered(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    sources: &[WorkspaceSource],
    work: &mut usize,
) -> Result<Option<Box<Inventory>>, Vec<Diagnostic>> {
    let Some(lexical) = selected(program, resolved, work)? else {
        return Ok(None);
    };
    // Demand: an exit-free closed module keeps its existing route.
    if !lexical {
        return Ok(None);
    }
    #[cfg(test)]
    assert!(
        resolved.functions.iter().all(|function| {
            crate::workspace_graph::loan_retention::authored_function(function, program, authored)
        }),
        "pairwise capture membership is the retention predicate"
    );
    #[cfg(not(test))]
    let _ = authored;
    let mut held = None;
    for source in sources {
        debit(
            work,
            source
                .path
                .len()
                .checked_add(program.path.len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(fail)?,
        )?;
        if source.path == program.path && held.replace(source).is_some() {
            return Err(fail());
        }
    }
    let held = held.ok_or_else(fail)?;
    // Digest scratch is priced before use; selection already reserved the
    // census cursor.
    reserve_builder_structure(body_digest::scratch_bytes())?;
    debit(
        work,
        revision_units(&program.path, &program.module, &held.source).ok_or_else(fail)?,
    )?;
    let revision = revision(&program.path, &program.module, &held.source);
    // Retained identity bytes: one debited addition per member.
    let count = resolved.functions.len();
    let mut ids = 0usize;
    for function in &resolved.functions {
        debit(work, 1)?;
        ids = ids
            .checked_add(function.id.as_str().len())
            .ok_or_else(capacity)?;
    }
    reserve_builder_structure(
        std::mem::size_of::<Inventory>()
            .checked_add(
                count
                    .checked_mul(std::mem::size_of::<Member>())
                    .ok_or_else(capacity)?,
            )
            .and_then(|n| n.checked_add(ids))
            .ok_or_else(capacity)?,
    )?;
    let mut members = Vec::with_capacity(count);
    if members.capacity() != count {
        return Err(refuse_excess::<Member>(members.capacity(), count));
    }
    // The authored origin is the paired AST function's own identity flag.
    for (source, function) in program.functions.iter().zip(&resolved.functions) {
        debit(work, 1)?;
        let Some(digest) = body_digest::typed_attempt_metered(
            function,
            &revision,
            Some(&mut |units| {
                *work = work.checked_sub(units)?;
                Some(())
            }),
        )
        .0
        else {
            return Ok(None);
        };
        members.push(Member {
            id: function.id.clone(),
            explicit: source.explicit_id,
            digest,
        });
    }
    let module = crate::bounded_output::budgeted_clone(&program.module);
    let path = crate::bounded_output::budgeted_clone(&program.path);
    if crate::bounded_output::active_refused() {
        return Err(capacity());
    }
    // Reception recomputes and charges its own digest work on its own ledger;
    // no capture-side work credit is retained or transferred.
    Ok(Some(Box::new(Inventory {
        module,
        path,
        members,
    })))
}

/// Selection phases of `capture_metered`. `Ok(Some(lexical))` only when every
/// pair is an authored member inside the closed family. A decline or a work
/// exhaustion is `Ok(None)`; only the cursor reservation can refuse.
fn selected(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    work: &mut usize,
) -> Result<Option<bool>, Vec<Diagnostic>> {
    // (1) Selector fields, exact pairwise identity and signature; no cursor.
    for (source, function) in program.functions.iter().zip(&resolved.functions) {
        if select(work, 1).is_err() {
            return Ok(None);
        }
        if !source.type_parameters.is_empty()
            || !source.effects.is_empty()
            || source.yields.is_some()
            || source.follows.is_some()
        {
            return Ok(None);
        }
        let Some(units) = key_units(&source.stable_id, function.id.as_str()) else {
            return Ok(None);
        };
        if select(work, units).is_err() || source.stable_id != function.id.as_str() {
            return Ok(None);
        }
        if !matches!(
            census::signature(function, &mut |units| select(work, units)),
            Ok(Some(_))
        ) {
            return Ok(None);
        }
    }
    // (2) Every signature is in the family: reserve the one cursor before its
    // first initialization, then walk every body.
    reserve_builder_structure(census::cursor_bytes())?;
    let mut lexical = false;
    for function in &resolved.functions {
        match census::body(function, &mut |units| select(work, units)) {
            Ok(Some(flags)) => lexical |= flags.lexical,
            Ok(None) | Err(()) => return Ok(None),
        }
    }
    Ok(Some(lexical))
}
