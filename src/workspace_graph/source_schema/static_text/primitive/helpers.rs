//! One authenticated borrowed cross-module scalar closure. Source facts select
//! no authority until the actual defining pointers and complete closure match.
use super::*;
use crate::cleanup_plan::{SelectedReplayPreparation, SelectedReplaySession};
use crate::workspace_graph::static_text::scalar_helpers;
type Frame<'a> = Option<(hir::lexical_exit::ChildCursor<'a>, bool)>;
const DEPTH: usize = 514;
mod graph;
pub(super) mod local;
mod types;
struct Member<'a> {
    module: &'a WorkspaceResolvedModule,
    function: &'a ResolvedFunction,
}
pub(super) struct Closure<'a> {
    local: Context<'a>,
    helpers: Vec<Member<'a>>,
    lexical: Vec<&'a ResolvedFunction>,
    count: usize,
    types: Vec<&'a ResolvedTypeDeclaration>,
    #[cfg(test)]
    walks: usize,
    #[cfg(test)]
    edges: usize,
}
impl ReplayContext for Closure<'_> {
    fn declarations(&self) -> &hir::DeclarationIndex {
        &self.local.index
    }
    fn types(&self) -> &[ResolvedTypeDeclaration] {
        &self.local.module.types
    }
    fn type_count(&self) -> usize {
        self.local.module.types.len() + self.types.len() + usize::from(self.local.vector.is_some())
    }
    /// Local types, borrowed defining-module types, then the canonical Vec
    /// rebuilt beside the shared index metadata.
    fn type_at(&self, index: usize) -> Option<&ResolvedTypeDeclaration> {
        self.local.module.types.get(index).or_else(|| {
            let held = index.checked_sub(self.local.module.types.len())?;
            self.types.get(held).copied().or_else(|| {
                self.local
                    .vector
                    .as_ref()
                    .filter(|_| held == self.types.len())
            })
        })
    }
    fn interfaces(&self) -> &[ResolvedInterface] {
        &[]
    }
    fn functions(&self) -> &[ResolvedFunction] {
        &self.local.module.functions
    }
    fn function_count(&self) -> usize {
        self.count
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
fn fact<'a>(
    id: &str,
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<&'a WorkspaceDeclarationFact, Vec<Diagnostic>> {
    for (key, value) in facts {
        charge(
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
                    if instance.is_some()
                        || (!type_arguments.is_empty()
                            && !(crate::vec_ops::by_id(callee.as_str()).is_some()
                                && matches!(type_arguments.as_slice(), [element]
                                    if hir::is_scalar_resolved_type(element)
                                    || matches!(element, hir::ResolvedType::String | hir::ResolvedType::Bytes)
                                    || matches!(element, hir::ResolvedType::Nominal { arguments, .. } if arguments.is_empty()))))
                    {
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
        || crate::vec_ops::by_id(id.as_str()).is_some()
}
// Imported helper signatures are scalar/view or argument-free borrowed records.
// Compare nominal identity bytes at the receiving equality, rather than relying
// on an earlier layout probe to pay for this separate comparison.
fn same_type(
    source: &hir::ResolvedType,
    current: &hir::ResolvedType,
    mut debit: impl FnMut(usize) -> Result<(), Vec<Diagnostic>>,
) -> Result<bool, Vec<Diagnostic>> {
    debit(1)?;
    if hir::is_scalar_resolved_type(source)
        || matches!(source, hir::ResolvedType::Str | hir::ResolvedType::SliceU8)
    {
        return Ok(source == current);
    }
    let (
        hir::ResolvedType::Nominal {
            declaration: a,
            arguments: aa,
        },
        hir::ResolvedType::Nominal {
            declaration: b,
            arguments: bb,
        },
    ) = (source, current)
    else {
        return Ok(false);
    };
    if !aa.is_empty() || !bb.is_empty() {
        return Ok(false);
    }
    debit(1)?;
    if a.as_str().len() != b.as_str().len() {
        return Ok(false);
    }
    for (a, b) in a.as_str().bytes().zip(b.as_str().bytes()) {
        debit(1)?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
fn authenticate_target<'a>(
    id: &hir::DeclarationId,
    modules: &'a [WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
    anchor: &ResolvedFunction,
) -> Result<Member<'a>, Vec<Diagnostic>> {
    let origin = fact(id.as_str(), facts, resources, anchor)?;
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
            && provider.replace(module).is_some()
        {
            return Err(fail());
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
    charge(
        resources,
        anchor,
        source
            .name
            .len()
            .checked_add(function.name.len())
            .ok_or_else(fail)?,
    )?;
    if !source.explicit
        || source.name != function.name
        || !same_type(&source.result, &function.return_type, |n| {
            charge(resources, anchor, n)
        })?
        || source.params.len() != function.params.len()
    {
        return Err(fail());
    }
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
            || !same_type(&source.ty, &current.ty, |n| charge(resources, anchor, n))?
            || source.ownership != current.ownership
        {
            return Err(fail());
        }
    }
    resources
        .reserve_array::<Frame<'_>>(anchor, DEPTH, "imported scalar helper profile cursor")
        .map_err(|_| fail())?;
    let mut work = 1_048_576;
    if !scalar_helpers::pure_records_metered(function, &module.types, &mut work, &mut |units| {
        charge(resources, anchor, units)
    })? {
        return Err(fail());
    }
    // The complete pure walk above independently excludes owned roots and
    // lexical exits. Such a provider has exactly the ordinary empty V1 loan
    // inventory; the attached plan is a consistency check, never authority.
    charge(
        resources,
        anchor,
        function
            .loan_plan
            .schema
            .len()
            .min(crate::loan_plan::LOAN_PLAN_SCHEMA_V1.len())
            .checked_add(1)
            .ok_or_else(fail)?,
    )?;
    if function.loan_plan.schema != crate::loan_plan::LOAN_PLAN_SCHEMA_V1
        || !function.loan_plan.loans.is_empty()
        || !function.loan_plan.endpoints.is_empty()
        || !function.loan_plan.edges.is_empty()
    {
        return Err(fail());
    }
    resources
        .reserve_array::<u8>(
            anchor,
            crate::workspace_graph::static_text::primitive::digest_scratch_bytes(),
            "imported scalar body digest scratch",
        )
        .map_err(|_| fail())?;
    if !scalar_helpers::matches_metered(inventory, source, function, &mut |units| {
        charge(resources, anchor, units).ok()
    }) {
        return Err(fail());
    }
    Ok(Member { module, function })
}

/// Keep the stable retained-source diagnostic. Owning tests name the refusing
/// closure boundary instead of inferring it from a generic G410; an earlier,
/// more specific help is never replaced. Production output is unchanged.
fn boundary<T>(
    answer: Result<T, Vec<Diagnostic>>,
    stage: &'static str,
) -> Result<T, Vec<Diagnostic>> {
    #[cfg(test)]
    let answer = answer.map_err(|errors| {
        errors
            .into_iter()
            .map(|error| {
                if error.help.is_some() {
                    error
                } else {
                    error.with_help(format!("imported scalar closure boundary: {stage}"))
                }
            })
            .collect()
    });
    #[cfg(not(test))]
    let _ = stage;
    answer
}
fn authenticate_inner<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    resources: &mut SelectedReplayPreparation,
) -> Result<Closure<'a>, Vec<Diagnostic>> {
    let anchor = module.functions.first().ok_or_else(fail)?;
    boundary(
        super::current_module(module, modules, resources, anchor),
        "current module",
    )?;
    resources
        .reserve_array::<Closure<'_>>(anchor, 1, "imported scalar closure owner")
        .map_err(|_| fail())?;
    let mut local = boundary(
        local::authenticate(module, facts, resources, anchor),
        "local source",
    )?;
    let types = boundary(
        types::discover(module, modules, facts, &mut local.index, resources, anchor),
        "defining types",
    )?;
    let graph = boundary(
        graph::discover(module, modules, facts, resources, anchor),
        "helper graph",
    )?;
    #[cfg(test)]
    let edges = boundary(
        graph::order(&graph, resources, anchor, |_| {}),
        "helper order",
    )?;
    #[cfg(not(test))]
    graph::order(&graph, resources, anchor, |_| {})?;
    #[cfg(test)]
    let walks = graph.walks;
    let graph::Graph { helpers, .. } = graph;
    let count = module
        .functions
        .len()
        .checked_add(helpers.len())
        .ok_or_else(fail)?;
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
    Ok(Closure {
        local,
        helpers,
        lexical,
        count,
        types,
        #[cfg(test)]
        walks,
        #[cfg(test)]
        edges,
    })
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
    if context.lexical.is_empty() {
        return Err(fail());
    }
    for function in &context.lexical {
        session.validate(function).map_err(|_| fail())?;
    }
    session.finish().map_err(|_| fail())?;
    Ok(())
}

#[cfg(test)]
mod tests;
