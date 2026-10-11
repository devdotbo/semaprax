//! One authenticated borrowed cross-module scalar closure. Source facts select
//! no authority until the actual defining pointers and complete closure match.
use super::*;
use crate::cleanup_plan::{SelectedReplayPreparation, SelectedReplaySession};
use crate::workspace_graph::static_text::scalar_helpers;
type Frame<'a> = Option<(hir::lexical_exit::ChildCursor<'a>, bool)>;
const DEPTH: usize = 514;
struct Member<'a> {
    module: &'a WorkspaceResolvedModule,
    function: &'a ResolvedFunction,
}
pub(super) struct Closure<'a> {
    local: Context<'a>,
    helpers: Vec<Member<'a>>,
}
impl ReplayContext for Closure<'_> {
    fn declarations(&self) -> &hir::DeclarationIndex {
        &self.local.index
    }
    fn types(&self) -> &[ResolvedTypeDeclaration] {
        &self.local.module.types
    }
    fn interfaces(&self) -> &[ResolvedInterface] {
        &[]
    }
    fn functions(&self) -> &[ResolvedFunction] {
        &self.local.module.functions
    }
    fn function_count(&self) -> usize {
        self.local.module.functions.len() + self.helpers.len()
    }
    fn function_at(&self, index: usize) -> Option<&ResolvedFunction> {
        self.local.module.functions.get(index).or_else(|| {
            index
                .checked_sub(self.local.module.functions.len())
                .and_then(|i| self.helpers.get(i).map(|m| m.function))
        })
    }
    fn function_templates(&self) -> &[ResolvedFunctionTemplate] {
        &[]
    }
    fn function_instances(&self) -> &[ResolvedFunctionInstance] {
        &[]
    }
    fn resolve_call_target(
        &self,
        id: &hir::DeclarationId,
        instance: Option<&hir::FunctionInstanceId>,
    ) -> Option<&ResolvedFunction> {
        if instance.is_some() {
            return None;
        }
        (0..self.function_count())
            .filter_map(|i| self.function_at(i))
            .find(|f| f.id == *id)
    }
}
fn charge(
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
    n: usize,
) -> Result<(), Vec<Diagnostic>> {
    resources
        .charge(anchor, n, "imported scalar helper authentication")
        .map_err(|_| fail())
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
fn calls(
    function: &ResolvedFunction,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
    mut call: impl FnMut(
        &hir::DeclarationId,
        &mut SelectedReplayPreparation,
    ) -> Result<(), Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    resources
        .reserve_array::<Frame<'_>>(anchor, DEPTH, "imported scalar helper body cursor")
        .map_err(|_| fail())?;
    let mut stack = [None; DEPTH];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        stack[0] = Some((hir::lexical_exit::ChildCursor::new(root), false));
        let mut depth = 1;
        while depth != 0 {
            charge(resources, anchor, 1)?;
            let (mut cursor, entered) = stack[depth - 1].take().ok_or_else(fail)?;
            if !entered {
                if let hir::ResolvedExprKind::Call {
                    callee,
                    instance,
                    type_arguments,
                    ..
                } = &cursor.expression().kind
                {
                    if instance.is_some() || !type_arguments.is_empty() {
                        return Err(fail());
                    }
                    call(callee, resources)?;
                }
            }
            if let Some(child) = cursor.next() {
                if depth == DEPTH {
                    return Err(fail());
                }
                stack[depth - 1] = Some((cursor, true));
                stack[depth] = Some((hir::lexical_exit::ChildCursor::new(child), false));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
    }
    Ok(())
}
fn builtin(id: &hir::DeclarationId) -> bool {
    crate::byte_ops::by_id(id.as_str()).is_some()
        || crate::string_ops::by_id(id.as_str()).is_some()
        || crate::str_ops::by_id(id.as_str()).is_some()
}
fn authenticate_target<'a>(
    id: &hir::DeclarationId,
    modules: &'a [WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<Member<'a>, Vec<Diagnostic>> {
    charge(
        resources,
        anchor,
        id.as_str().len().checked_add(1).ok_or_else(fail)?,
    )?;
    let origin = facts.get(id.as_str()).ok_or_else(fail)?;
    if origin.kind != hir::DeclarationKind::Function
        || origin.owner.is_some()
        || origin.origin != hir::IdentityOrigin::Explicit
    {
        return Err(fail());
    }
    let mut provider = None;
    charge(resources, anchor, modules.len())?;
    for module in modules {
        charge(
            resources,
            anchor,
            module
                .module
                .len()
                .checked_add(module.path.len())
                .and_then(|n| n.checked_add(origin.module.as_ref().map_or(0, String::len)))
                .and_then(|n| n.checked_add(origin.path.as_ref().map_or(0, String::len)))
                .ok_or_else(fail)?,
        )?;
        if Some(module.module.as_str()) == origin.module.as_deref()
            && Some(module.path.as_str()) == origin.path.as_deref()
        {
            if provider.replace(module).is_some() {
                return Err(fail());
            }
        }
    }
    let module = provider.ok_or_else(fail)?;
    let inventory = module.scalar_helpers.as_deref().ok_or_else(fail)?;
    charge(
        resources,
        anchor,
        inventory
            .functions
            .len()
            .checked_add(module.functions.len())
            .ok_or_else(fail)?,
    )?;
    let mut selected = None;
    for function in &module.functions {
        charge(
            resources,
            anchor,
            function
                .id
                .as_str()
                .len()
                .checked_add(id.as_str().len())
                .ok_or_else(fail)?,
        )?;
        if function.id == *id && selected.replace(function).is_some() {
            return Err(fail());
        }
    }
    let function = selected.ok_or_else(fail)?;
    let mut expected = None;
    for fact in &inventory.functions {
        charge(
            resources,
            anchor,
            fact.id
                .as_str()
                .len()
                .checked_add(id.as_str().len())
                .ok_or_else(fail)?,
        )?;
        if fact.id == *id && expected.replace(fact).is_some() {
            return Err(fail());
        }
    }
    let source = expected.ok_or_else(fail)?;
    if !source.explicit
        || source.name != function.name
        || source.result != function.return_type
        || source.params.len() != function.params.len()
    {
        return Err(fail());
    }
    charge(
        resources,
        anchor,
        source
            .name
            .len()
            .checked_add(function.name.len())
            .ok_or_else(fail)?,
    )?;
    for (source, current) in source.params.iter().zip(&function.params) {
        charge(
            resources,
            anchor,
            source
                .id
                .as_str()
                .len()
                .checked_add(current.id.as_str().len())
                .and_then(|n| n.checked_add(source.name.len()))
                .and_then(|n| n.checked_add(current.name.len()))
                .ok_or_else(fail)?,
        )?;
        if source.id != current.id
            || source.name != current.name
            || source.ty != current.ty
            || current.ownership != hir::OwnershipMode::Value
        {
            return Err(fail());
        }
    }
    resources
        .reserve_array::<Frame<'_>>(anchor, DEPTH, "imported scalar helper profile cursor")
        .map_err(|_| fail())?;
    let mut work = 1_048_576;
    if !scalar_helpers::pure(function, &mut work)? {
        return Err(fail());
    }
    charge(
        resources,
        anchor,
        1_048_576usize.checked_sub(work).ok_or_else(fail)?,
    )?;
    let spent = scalar_helpers::match_work(inventory, source, function).ok_or_else(fail)?;
    charge(resources, anchor, spent)?;
    Ok(Member { module, function })
}

fn authenticate_inner<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
) -> Result<Closure<'a>, Vec<Diagnostic>> {
    let anchor = module.functions.first().ok_or_else(fail)?;
    resources
        .reserve_array::<Closure<'_>>(anchor, 1, "imported scalar closure owner")
        .map_err(|_| fail())?;
    // Existing local source/layout checks are retained; account each digest and
    // constructor scan in the same non-clone work owner before replay.
    let inventory = module
        .static_text
        .as_deref()
        .and_then(|i| i.lexical.as_deref())
        .ok_or_else(fail)?;
    for (source, function) in inventory.functions.iter().zip(&module.functions) {
        let work = crate::workspace_graph::static_text::primitive::body_match_work(
            inventory, source, function,
        )
        .ok_or_else(fail)?;
        charge(
            resources,
            anchor,
            work.checked_mul(2)
                .and_then(|n| n.checked_add(function.params.len()))
                .ok_or_else(fail)?,
        )?;
    }
    let local = super::authenticate(
        module,
        facts,
        resources.physical(anchor).map_err(|_| fail())?,
    )?;
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
    let mut charged = vec![0u8; modules.len()];
    let mut helpers: Vec<Member<'a>> = Vec::with_capacity(maximum);
    if helpers.capacity() != maximum {
        return Err(fail());
    }
    let mut processed = 0;
    while processed < module.functions.len() + helpers.len() {
        let function = if processed < module.functions.len() {
            &module.functions[processed]
        } else {
            helpers[processed - module.functions.len()].function
        };
        let provider = processed >= module.functions.len();
        calls(function, resources, anchor, |id, resources| {
            if builtin(id) {
                return if provider { Err(fail()) } else { Ok(()) };
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
                helpers.push(member);
            }
            Ok(())
        })?;
        processed = processed.checked_add(1).ok_or_else(fail)?;
    }
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
    let mut indegree = vec![0usize; count];
    let mut pending = Vec::with_capacity(count);
    let mut edges = Vec::new();
    for (from, function) in module
        .functions
        .iter()
        .chain(helpers.iter().map(|m| m.function))
        .enumerate()
    {
        calls(function, resources, anchor, |id, resources| {
            if builtin(id) {
                return Ok(());
            }
            let to = lookup(id, module, &helpers, resources, anchor)?.ok_or_else(fail)?;
            resources
                .reserve_vec(anchor, &mut edges, 1, "imported scalar closure edges")
                .map_err(|_| fail())?;
            edges.push((from, to));
            indegree[to] = indegree[to].checked_add(1).ok_or_else(fail)?;
            Ok(())
        })?;
    }
    for (i, n) in indegree.iter().enumerate() {
        charge(resources, anchor, 1)?;
        if *n == 0 {
            pending.push(i);
        }
    }
    let mut visited = 0usize;
    while let Some(from) = pending.pop() {
        visited = visited.checked_add(1).ok_or_else(fail)?;
        for (source, to) in &edges {
            charge(resources, anchor, 1)?;
            if *source == from {
                indegree[*to] = indegree[*to].checked_sub(1).ok_or_else(fail)?;
                if indegree[*to] == 0 {
                    if pending.len() == count {
                        return Err(fail());
                    }
                    pending.push(*to);
                }
            }
        }
    }
    if visited != count {
        return Err(fail());
    }
    let mut local = local;
    for member in &helpers {
        charge(
            resources,
            anchor,
            member
                .function
                .id
                .as_str()
                .len()
                .checked_add(member.function.name.len())
                .and_then(|n| n.checked_add(member.function.params.len() + 1))
                .ok_or_else(fail)?,
        )?;
        local
            .index
            .append_scalar_helper_replay(
                member.function,
                resources.physical(anchor).map_err(|_| fail())?,
            )
            .map_err(|_| fail())?;
    }
    Ok(Closure { local, helpers })
}
pub(super) fn authenticate<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
) -> Result<Closure<'a>, Vec<Diagnostic>> {
    let result = authenticate_inner(module, modules, facts, resources);
    if result.is_err() {
        resources.fail();
    }
    result
}

pub(super) fn replay(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    physical: crate::cleanup_plan::ReplayPhysicalReservation,
) -> Result<(), Vec<Diagnostic>> {
    let anchor = module.functions.first().ok_or_else(fail)?;
    let mut resources = SelectedReplayPreparation::new(anchor, physical).map_err(|_| fail())?;
    let context = match authenticate(module, modules, facts, &mut resources) {
        Ok(context) => context,
        Err(error) => {
            resources.fail();
            return Err(error);
        }
    };
    let mut session =
        SelectedReplaySession::from_preparation(&context, anchor, resources).map_err(|_| fail())?;
    for helper in &context.helpers {
        session
            .validate_scalar(helper.function)
            .map_err(|_| fail())?;
    }
    let mut found = false;
    for function in &module.functions {
        if hir::lexical_exit::requires(function).map_err(|e| vec![e])? {
            session.validate(function).map_err(|_| fail())?;
            found = true;
        }
    }
    if !found {
        return Err(fail());
    }
    session.finish().map_err(|_| fail())?;
    Ok(())
}

#[cfg(test)]
mod tests;
