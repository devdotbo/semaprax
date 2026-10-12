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
pub(in crate::workspace_graph) fn member_matches_metered(
    member: &Member,
    function: &hir::ResolvedFunction,
    revision: &[u8; 32],
    debit: &mut dyn FnMut(usize) -> Option<()>,
) -> bool {
    member.id == function.id
        && body_digest::typed_attempt_metered(function, revision, Some(debit)).0
            == Some(member.digest)
}

fn debit(work: &mut usize, units: usize) -> Result<(), Vec<Diagnostic>> {
    *work = work.checked_sub(units).ok_or_else(fail)?;
    Ok(())
}

/// Called by both retention routes before any output collection is filtered
/// or moved. Membership is the retention predicate itself, so the retained
/// function sequence is exactly the captured member sequence.
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
    // existing route. These are O(1) tests and charge no builder bytes.
    if !program.module_uses.is_empty()
        || !program.agents.is_empty()
        || !program.interfaces.is_empty()
        || !program.protocols.is_empty()
        || !program.implementations.is_empty()
        || !program.permits.is_empty()
        || !program.types.is_empty()
        || program.functions.is_empty()
        || !resolved.interfaces.is_empty()
        || !resolved.permits.is_empty()
        || !resolved.agents.is_empty()
        || !resolved.function_instances.is_empty()
        || program.functions.iter().any(|function| {
            !function.type_parameters.is_empty()
                || !function.effects.is_empty()
                || function.yields.is_some()
                || function.follows.is_some()
        })
    {
        return Ok(None);
    }
    // Demand selection only: a retained authored member with an authored exit.
    // This bounded borrowed probe allocates nothing and grants nothing; the
    // metered census below re-derives the lexical flag it relies on.
    let demanded = resolved.functions.iter().any(|function| {
        crate::workspace_graph::loan_retention::authored_function(function, program, authored)
            && function
                .requires
                .iter()
                .chain(std::iter::once(&function.body))
                .chain(&function.ensures)
                .any(hir::lexical_exit::contains)
    });
    if !demanded {
        return Ok(None);
    }
    let mut work = WORK;
    let mut held = None;
    for source in sources {
        debit(
            &mut work,
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
    // Digest scratch and the fixed census cursor are priced before use.
    reserve_builder_structure(
        body_digest::scratch_bytes()
            .checked_add(census::cursor_bytes())
            .ok_or_else(capacity)?,
    )?;
    debit(
        &mut work,
        revision_units(&program.path, &program.module, &held.source).ok_or_else(fail)?,
    )?;
    let revision = revision(&program.path, &program.module, &held.source);
    // Complete member census and exact authored origin, before allocation.
    let mut count = 0usize;
    let mut lexical = false;
    for function in &resolved.functions {
        debit(&mut work, 1)?;
        if !crate::workspace_graph::loan_retention::authored_function(function, program, authored)
        {
            continue;
        }
        let origin = authored.get(function.id.as_str()).ok_or_else(fail)?;
        let source = origin.function.ok_or_else(fail)?;
        debit(
            &mut work,
            origin
                .path
                .len()
                .checked_add(origin.module.len())
                .and_then(|n| n.checked_add(source.stable_id.len()))
                .and_then(|n| n.checked_add(function.id.as_str().len()))
                .and_then(|n| n.checked_add(program.functions.len()))
                .and_then(|n| n.checked_add(1))
                .ok_or_else(fail)?,
        )?;
        if origin.path != program.path
            || origin.module != program.module
            || origin.explicit != source.explicit_id
            || source.stable_id != function.id.as_str()
            || !program
                .functions
                .iter()
                .any(|authored| std::ptr::eq(authored, source))
        {
            return Err(fail());
        }
        let Some(flags) = census::function(function, &mut |units| debit(&mut work, units))? else {
            return Ok(None);
        };
        lexical |= flags.lexical;
        count = count.checked_add(1).ok_or_else(fail)?;
    }
    // Each authored function has one unique identity; equal cardinality is the
    // exact bijection between authored and retained members.
    if count != program.functions.len() {
        return Err(fail());
    }
    if !lexical {
        return Ok(None);
    }
    let ids = resolved
        .functions
        .iter()
        .filter(|function| {
            crate::workspace_graph::loan_retention::authored_function(function, program, authored)
        })
        .try_fold(0usize, |total, function| {
            total.checked_add(function.id.as_str().len())
        })
        .ok_or_else(capacity)?;
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
        return Err(capacity());
    }
    for function in &resolved.functions {
        if !crate::workspace_graph::loan_retention::authored_function(function, program, authored)
        {
            continue;
        }
        let origin = authored.get(function.id.as_str()).ok_or_else(fail)?;
        let Some(digest) = body_digest::typed_attempt_metered(
            function,
            &revision,
            Some(&mut |units| {
                work = work.checked_sub(units)?;
                Some(())
            }),
        )
        .0
        else {
            return Ok(None);
        };
        members.push(Member {
            id: function.id.clone(),
            explicit: origin.explicit,
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
