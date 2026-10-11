//! Saved pre-#763 complete authentication entry: linear current lookup per
//! call, a second walk of every selected body for edges, and a full edge-vector
//! scan per Kahn vertex, with every original debit and reservation in place.
//! It shares only the unchanged local, target, body-cursor and builtin code with
//! production, records its observable shape without charging for the record,
//! and hands the same `Closure` to the unchanged session through final Drop.
use super::*;

pub(super) struct Shape {
    pub(super) functions: Vec<*const ResolvedFunction>,
    pub(super) edges: Vec<(usize, usize)>,
    pub(super) order: Vec<usize>,
    pub(super) acyclic: bool,
    pub(super) walks: usize,
    pub(super) visits: usize,
}
fn lookup<'a>(
    id: &hir::DeclarationId,
    module: &'a WorkspaceResolvedModule,
    helpers: &'a [Member<'a>],
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<Option<usize>, Vec<Diagnostic>> {
    let mut found = None;
    for (i, function) in module
        .functions
        .iter()
        .chain(helpers.iter().map(|m| m.function))
        .enumerate()
    {
        charge(
            resources,
            anchor,
            id.as_str()
                .len()
                .checked_add(function.id.as_str().len())
                .ok_or_else(fail)?,
        )?;
        if function.id == *id {
            if found.replace(i).is_some() {
                return Err(fail());
            }
        }
    }
    Ok(found)
}

/// The saved closure discovery and Kahn order with all original debits. The
/// shape record is kept outside the ledger and is complete only on success
/// or at a cycle refusal.
fn closure<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
    shape: &mut Shape,
) -> Result<Vec<Member<'a>>, Vec<Diagnostic>> {
    let maximum = modules
        .iter()
        .try_fold(0usize, |n, m| n.checked_add(m.functions.len()))
        .ok_or_else(fail)?;
    charge(resources, anchor, modules.len())?;
    resources
        .reserve_array::<Member<'_>>(anchor, maximum, "imported scalar closure members")
        .map_err(|_| fail())?;
    resources
        .reserve_array::<u8>(
            anchor,
            modules.len(),
            "imported scalar defining source debits",
        )
        .map_err(|_| fail())?;
    charge(resources, anchor, modules.len())?;
    let mut charged = vec![0u8; modules.len()];
    let mut helpers: Vec<Member<'a>> = Vec::with_capacity(maximum);
    if helpers.capacity() != maximum {
        return Err(fail());
    }
    let mut processed = 0;
    loop {
        let total = module
            .functions
            .len()
            .checked_add(helpers.len())
            .ok_or_else(fail)?;
        if processed >= total {
            break;
        }
        let function = if processed < module.functions.len() {
            &module.functions[processed]
        } else {
            helpers[processed - module.functions.len()].function
        };
        let provider = processed >= module.functions.len();
        shape.walks += 1;
        calls(function, resources, anchor, |id, resources| {
            if builtin(id) {
                return if provider && !hir::borrowed_scalar::builtin(id.as_str()) {
                    Err(fail())
                } else {
                    Ok(())
                };
            }
            if lookup(id, module, &helpers, resources, anchor)?.is_none() {
                let member = authenticate_target(id, modules, facts, resources, anchor)?;
                if std::ptr::eq(member.module, module) {
                    return Err(fail());
                }
                let ordinal = modules
                    .iter()
                    .position(|m| std::ptr::eq(m, member.module))
                    .ok_or_else(fail)?;
                charge(resources, anchor, modules.len())?;
                if charged[ordinal] == 0 {
                    charge(
                        resources,
                        anchor,
                        member
                            .module
                            .scalar_helpers
                            .as_deref()
                            .ok_or_else(fail)?
                            .capture_work,
                    )?;
                    charged[ordinal] = 1;
                }
                if helpers.len() == maximum {
                    return Err(fail());
                }
                charge(resources, anchor, 1)?;
                helpers.push(member);
            }
            Ok(())
        })?;
        processed = processed.checked_add(1).ok_or_else(fail)?;
    }
    shape.functions = module
        .functions
        .iter()
        .chain(helpers.iter().map(|m| m.function))
        .map(|f| f as *const ResolvedFunction)
        .collect();
    let count = module
        .functions
        .len()
        .checked_add(helpers.len())
        .ok_or_else(fail)?;
    resources
        .reserve_array::<usize>(anchor, count, "imported scalar closure indegrees")
        .map_err(|_| fail())?;
    resources
        .reserve_array::<usize>(anchor, count, "imported scalar closure ready frontier")
        .map_err(|_| fail())?;
    charge(resources, anchor, count)?;
    let mut indegree = vec![0usize; count];
    let mut pending = Vec::with_capacity(count);
    let mut edges = Vec::new();
    for (from, function) in module
        .functions
        .iter()
        .chain(helpers.iter().map(|m| m.function))
        .enumerate()
    {
        shape.walks += 1;
        calls(function, resources, anchor, |id, resources| {
            if builtin(id) {
                return Ok(());
            }
            let to = lookup(id, module, &helpers, resources, anchor)?.ok_or_else(fail)?;
            resources
                .reserve_vec(anchor, &mut edges, 1, "imported scalar closure edges")
                .map_err(|_| fail())?;
            charge(resources, anchor, 1)?;
            edges.push((from, to));
            indegree[to] = indegree[to].checked_add(1).ok_or_else(fail)?;
            Ok(())
        })?;
    }
    shape.edges.clone_from(&edges);
    for (i, n) in indegree.iter().enumerate() {
        charge(resources, anchor, 1)?;
        if *n == 0 {
            charge(resources, anchor, 1)?;
            pending.push(i);
        }
    }
    let mut visited = 0usize;
    while let Some(from) = pending.pop() {
        charge(resources, anchor, 1)?;
        visited = visited.checked_add(1).ok_or_else(fail)?;
        shape.order.push(from);
        for (source, to) in &edges {
            charge(resources, anchor, 1)?;
            shape.visits += 1;
            if *source == from {
                indegree[*to] = indegree[*to].checked_sub(1).ok_or_else(fail)?;
                if indegree[*to] == 0 {
                    if pending.len() == count {
                        return Err(fail());
                    }
                    charge(resources, anchor, 1)?;
                    pending.push(*to);
                }
            }
        }
    }
    if visited != count {
        return Err(fail());
    }
    shape.acyclic = true;
    Ok(helpers)
}
fn empty() -> Shape {
    Shape {
        functions: Vec::new(),
        edges: Vec::new(),
        order: Vec::new(),
        acyclic: false,
        walks: 0,
        visits: 0,
    }
}

/// Saved complete `authenticate`: owner reservation, unchanged local source
/// authentication, saved closure, replay index growth and lexical members,
/// with the original sticky failure on any refusal.
pub(super) fn authenticate<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
) -> Result<(Closure<'a>, Shape), Vec<Diagnostic>> {
    let result = authenticate_inner(module, modules, facts, resources);
    if result.is_err() {
        resources.fail();
    }
    result
}
fn authenticate_inner<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
) -> Result<(Closure<'a>, Shape), Vec<Diagnostic>> {
    let anchor = module.functions.first().ok_or_else(fail)?;
    resources
        .reserve_array::<Closure<'_>>(anchor, 1, "imported scalar closure owner")
        .map_err(|_| fail())?;
    let local = local::authenticate(module, facts, resources, anchor)?;
    let mut shape = empty();
    let helpers = closure(module, modules, facts, resources, anchor, &mut shape)?;
    let count = module
        .functions
        .len()
        .checked_add(helpers.len())
        .ok_or_else(fail)?;
    let mut local = local;
    for member in &helpers {
        resources
            .construct(anchor, |physical, debit| {
                local
                    .index
                    .append_scalar_helper_replay_metered(member.function, physical, debit)
            })
            .map_err(|_| fail())?;
    }
    resources
        .reserve_array::<&ResolvedFunction>(
            anchor,
            module.functions.len(),
            "imported consumer lexical members",
        )
        .map_err(|_| fail())?;
    let mut lexical = Vec::with_capacity(module.functions.len());
    if lexical.capacity() != module.functions.len() {
        return Err(fail());
    }
    for function in &module.functions {
        if local::requires(function, resources, anchor)? {
            charge(resources, anchor, 1)?;
            lexical.push(function);
        }
    }
    let edges = shape.edges.len();
    let walks = shape.walks;
    Ok((
        Closure {
            local,
            helpers,
            lexical,
            count,
            walks,
            edges,
        },
        shape,
    ))
}

/// Saved graph alone under its own wide owner, for direct HIR-mutant controls
/// that deliberately bypass consumer source authentication.
pub(super) fn run(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
) -> Result<Shape, Vec<Diagnostic>> {
    let anchor = module.functions.first().ok_or_else(fail)?;
    let mut resources =
        SelectedReplayPreparation::with_limits(anchor, 32_000_000, 1 << 28).map_err(|_| fail())?;
    let mut shape = empty();
    match closure(module, modules, facts, &mut resources, anchor, &mut shape) {
        Ok(_) => Ok(shape),
        // After a completed discovery the second walk cannot miss and the
        // wide owner cannot exhaust, so the only refusal is the cycle verdict.
        Err(error) if shape.functions.is_empty() => Err(error),
        Err(_) => {
            assert!(!shape.acyclic && shape.order.len() < shape.functions.len());
            Ok(shape)
        }
    }
}
