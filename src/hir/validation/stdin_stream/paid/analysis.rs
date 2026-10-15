//! Bounded, source-independent streaming lifetime and helper derivation.
use crate::diagnostic::Diagnostic;
use crate::hir::ordinary_scan;
use crate::hir::{
    self, DeclarationId, ResolvedExpr, ResolvedExprKind, ResolvedFunction, ResolvedProgram,
    ResolvedStatement,
};
use crate::stdin_stream_ops as stream;
use std::collections::{BTreeMap, BTreeSet};

const MAX_WORK: usize = 1_000_000;
use stream::analysis::FunctionFacts;
fn error(message: &'static str) -> Diagnostic {
    Diagnostic::io_static("SPX-H006", message)
}
fn charge(work: &mut usize) -> Result<(), Diagnostic> {
    *work += 1;
    if *work > MAX_WORK {
        Err(error("streaming admission exceeds bounded work"))
    } else {
        Ok(())
    }
}

pub(crate) fn derive(
    program: &ResolvedProgram,
) -> Result<BTreeMap<DeclarationId, FunctionFacts>, Diagnostic> {
    if !super::program_uses(program)? {
        return Ok(BTreeMap::new());
    }
    if !program.agents.is_empty() {
        return Err(error(
            "streaming stdin does not compose with agent execution",
        ));
    }
    if program.functions.len() > crate::byte_data_capacity::MAX_FUNCTIONS {
        return Err(error("streaming admission exceeds function capacity"));
    }
    let mut functions = BTreeMap::new();
    for f in &program.functions {
        insert_map(&mut functions, f.id.clone(), f)?;
    }
    let mut reverse = BTreeMap::<DeclarationId, BTreeSet<DeclarationId>>::new();
    let mut relevant = BTreeSet::new();
    let mut work = 0;
    for function in &program.functions {
        if stream::is_reader(&function.return_type)
            || function.params.iter().any(|p| stream::is_reader(&p.ty))
        {
            insert_set(&mut relevant, function.id.clone())?;
        }
        let mut pending = Vec::new();
        push(&mut pending, &function.body)?;
        while let Some(expression) = pending.pop() {
            charge(&mut work)?;
            if stream::is_reader(&expression.ty)
                || matches!(&expression.kind, ResolvedExprKind::HostCommandCall(call) if stream::is_host(call.operation))
                || matches!(&expression.kind, ResolvedExprKind::BorrowPlace { operation, .. } if operation.as_str() == stream::CHUNK_ID)
                || matches!(&expression.kind, ResolvedExprKind::Call { callee, .. } if callee.as_str() == stream::EOF_ID)
            {
                insert_set(&mut relevant, function.id.clone())?;
            }
            if let ResolvedExprKind::Call {
                callee,
                instance: None,
                ..
            } = &expression.kind
            {
                if functions.contains_key(callee) {
                    if !reverse.contains_key(callee) {
                        insert_map(&mut reverse, callee.clone(), BTreeSet::new())?;
                    }
                    insert_set(
                        reverse.get_mut(callee).expect("just indexed"),
                        function.id.clone(),
                    )?;
                }
            }
            if matches!(
                expression.kind,
                ResolvedExprKind::Closure { .. }
                    | ResolvedExprKind::Invoke { .. }
                    | ResolvedExprKind::FunctionReference { .. }
            ) {
                return Err(error(
                    "streaming stdin does not compose with function values",
                ));
            }
            if let ResolvedExprKind::Closure { body, .. } = &expression.kind {
                if super::expression_uses(body)? {
                    return Err(error("streaming stdin cannot execute in a closure"));
                }
            }
            ordinary_scan::children(expression, &mut |child| push(&mut pending, child))?;
        }
    }
    let mut pending = Vec::new();
    for id in &relevant {
        push(&mut pending, id.clone())?;
    }
    while let Some(callee) = pending.pop() {
        for caller in reverse.get(&callee).into_iter().flatten() {
            charge(&mut work)?;
            if insert_set(&mut relevant, caller.clone())? {
                push(&mut pending, caller.clone())?;
            }
        }
    }
    let mut facts = BTreeMap::new();
    let mut active = BTreeSet::new();
    for root in &relevant {
        let mut pending = Vec::new();
        push(&mut pending, (root.clone(), false))?;
        while let Some((id, finishing)) = pending.pop() {
            charge(&mut work)?;
            if facts.contains_key(&id) {
                continue;
            }
            let function = functions
                .get(&id)
                .ok_or_else(|| error("streaming helper identity is not indexed"))?;
            if finishing {
                let open_bound = expression_bound(&function.body, &facts, &mut work)?;
                let forwarding_parameter = if stream::is_reader(&function.return_type) {
                    if !stream::resolved_forward_signature(function) {
                        return Err(error(
                            "streaming reader result requires one owned forwarding parameter",
                        ));
                    }
                    authenticate_forward_tail(program, function)?;
                    Some(function.params[0].id.clone())
                } else {
                    None
                };
                if function
                    .params
                    .iter()
                    .any(|p| stream::is_reader(&p.ty) && p.ownership == hir::OwnershipMode::Own)
                    && forwarding_parameter.is_none()
                {
                    return Err(error("owned reader calls must forward the same reader"));
                }
                if function
                    .params
                    .iter()
                    .any(|param| stream::is_reader(&param.ty))
                    && open_bound != 0
                {
                    return Err(error(
                        "reader forwarding helpers cannot open a second reader",
                    ));
                }
                let mut entry = FunctionFacts {
                    open_bound,
                    forwarding_parameter,
                    opens: Vec::new(),
                    nexts: Vec::new(),
                    chunks: Vec::new(),
                    eofs: Vec::new(),
                };
                let mut expressions = Vec::new();
                push(&mut expressions, &function.body)?;
                while let Some(expression) = expressions.pop() {
                    charge(&mut work)?;
                    match &expression.kind {
                        ResolvedExprKind::HostCommandCall(call)
                            if call.operation
                                == hir::ResolvedHostCommandOperation::StdinStreamOpen =>
                        {
                            push(&mut entry.opens, expression.id.clone())?
                        }
                        ResolvedExprKind::HostCommandCall(call)
                            if call.operation
                                == hir::ResolvedHostCommandOperation::StdinStreamNext =>
                        {
                            push(&mut entry.nexts, expression.id.clone())?
                        }
                        ResolvedExprKind::BorrowPlace { operation, .. }
                            if operation.as_str() == stream::CHUNK_ID =>
                        {
                            push(&mut entry.chunks, expression.id.clone())?
                        }
                        ResolvedExprKind::Call { callee, .. }
                            if callee.as_str() == stream::EOF_ID =>
                        {
                            push(&mut entry.eofs, expression.id.clone())?
                        }
                        _ => {}
                    }
                    ordinary_scan::children(expression, &mut |child| {
                        push(&mut expressions, child)
                    })?;
                }
                insert_map(&mut facts, id.clone(), entry)?;
                active.remove(&id);
                continue;
            }
            if !insert_set(&mut active, id.clone())? {
                return Err(error("streaming stdin call graph must be acyclic"));
            }
            push(&mut pending, (id.clone(), true))?;
            let mut expressions = Vec::new();
            push(&mut expressions, &function.body)?;
            while let Some(expression) = expressions.pop() {
                charge(&mut work)?;
                if let ResolvedExprKind::Call {
                    callee,
                    instance: None,
                    ..
                } = &expression.kind
                {
                    if relevant.contains(callee) && !facts.contains_key(callee) {
                        push(&mut pending, (callee.clone(), false))?;
                    }
                }
                ordinary_scan::children(expression, &mut |child| push(&mut expressions, child))?;
            }
        }
    }
    Ok(facts)
}
fn authenticate_forward_tail(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    let mut tail = &function.body;
    while let ResolvedExprKind::Block { tail: next, .. } = &tail.kind {
        tail = next;
    }
    let owner = &function.params[0].id;
    if matches!(&tail.kind, ResolvedExprKind::Place(place) if &place.root == owner && place.projections.is_empty())
    {
        return Ok(());
    }
    if !stream::hir_reopen(tail, owner) {
        return Err(error(
            "reader forwarding tail must return its exact parameter or successor",
        ));
    }
    if let ResolvedExprKind::Call { callee, .. } = &tail.kind {
        if !program
            .resolve_call_target(callee, None)
            .is_some_and(stream::resolved_forward_signature)
        {
            return Err(error(
                "reader forwarding tail calls an unauthenticated carrier helper",
            ));
        }
    }
    Ok(())
}
fn expression_bound(
    expression: &ResolvedExpr,
    functions: &BTreeMap<DeclarationId, FunctionFacts>,
    work: &mut usize,
) -> Result<u8, Diagnostic> {
    enum Frame<'a> {
        Enter(&'a ResolvedExpr),
        Finish(&'a ResolvedExpr, usize),
    }
    let mut pending = Vec::new();
    push(&mut pending, Frame::Enter(expression))?;
    let mut values = Vec::<u8>::new();
    while let Some(frame) = pending.pop() {
        charge(work)?;
        match frame {
            Frame::Enter(expression) => {
                let mut children = Vec::new();
                ordinary_scan::children(expression, &mut |child| push(&mut children, child))?;
                push(&mut pending, Frame::Finish(expression, children.len()))?;
                for child in children {
                    push(&mut pending, Frame::Enter(child))?;
                }
            }
            Frame::Finish(expression, count) => {
                let offset = values
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| error("streaming bound child accounting disagrees"))?;
                let children = &values[offset..];
                let cost = match &expression.kind {
                    ResolvedExprKind::If { .. } => {
                        children[0].saturating_add(children[1].max(children[2]))
                    }
                    ResolvedExprKind::Match { arms, .. } => {
                        let mut index = 1;
                        let mut guards = 0u8;
                        let mut arm_max = 0u8;
                        for arm in arms {
                            if arm.guard.is_some() {
                                guards = guards.saturating_add(children[index]);
                                index += 1;
                            }
                            arm_max = arm_max.max(children[index]);
                            index += 1;
                        }
                        children[0].saturating_add(guards).saturating_add(arm_max)
                    }
                    ResolvedExprKind::HostCommandCall(call)
                        if call.operation == hir::ResolvedHostCommandOperation::StdinStreamOpen =>
                    {
                        children.iter().copied().fold(1, u8::saturating_add)
                    }
                    ResolvedExprKind::Call { callee, .. } => children.iter().copied().fold(
                        functions.get(callee).map_or(0, |facts| facts.open_bound),
                        u8::saturating_add,
                    ),
                    ResolvedExprKind::Block { statements, .. }
                    | ResolvedExprKind::VecConstructor { statements, .. } => {
                        let mut child = 0;
                        for statement in statements {
                            let count = statement.child_count();
                            if matches!(statement, ResolvedStatement::While { .. })
                                && children[child..child + count]
                                    .iter()
                                    .any(|bound| *bound != 0)
                            {
                                return Err(error("streaming stdin Open cannot execute in a loop"));
                            }
                            child += count;
                        }
                        children.iter().copied().fold(0, u8::saturating_add)
                    }
                    _ => children.iter().copied().fold(0, u8::saturating_add),
                };
                if cost > 1 {
                    return Err(error(
                        "streaming stdin Open may execute at most once per invocation path",
                    ));
                }
                values.truncate(offset);
                push(&mut values, cost)?;
            }
        }
    }
    Ok(values.pop().unwrap_or(0))
}

fn insert_map<K: Ord, V>(
    map: &mut BTreeMap<K, V>,
    key: K,
    value: V,
) -> Result<Option<V>, Diagnostic> {
    ordinary_scan::fund(Some(0))?;
    if !map.contains_key(&key) {
        ordinary_scan::fund(crate::cleanup_plan::replay_btree_node_upper::<K, V>())?;
    }
    Ok(map.insert(key, value))
}
fn insert_set<K: Ord>(set: &mut BTreeSet<K>, key: K) -> Result<bool, Diagnostic> {
    ordinary_scan::fund(Some(0))?;
    if !set.contains(&key) {
        ordinary_scan::fund(crate::cleanup_plan::replay_btree_node_upper::<K, ()>())?;
    }
    Ok(set.insert(key))
}
fn push<T>(values: &mut Vec<T>, value: T) -> Result<(), Diagnostic> {
    ordinary_scan::fund(Some(0))?;
    if values.len() == values.capacity() {
        let capacity = values.capacity().checked_mul(2).map(|n| n.max(4));
        let Some(capacity) = capacity else {
            return ordinary_scan::fund(None);
        };
        ordinary_scan::fund(capacity.checked_mul(std::mem::size_of::<T>()))?;
        let mut next = Vec::new();
        if next.try_reserve_exact(capacity).is_err() || next.capacity() != capacity {
            crate::bounded_output::refuse_active_required(usize::MAX);
            return Err(error("ordinary HIR scan backing allocation failed"));
        }
        next.append(values);
        *values = next;
    }
    values.push(value);
    Ok(())
}
