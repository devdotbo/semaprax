//! Borrowed retained-body authority for the selective byte-helper graph schema.
//! No whole-program clone and no authority from an attached schema token.
use super::*;
use hir::{OwnershipMode, ResolvedExprKind, ResolvedType};
mod proofs;
#[cfg(test)]
mod tests;
mod type_authority;
pub(in crate::workspace_graph) use proofs::Proofs;

fn target<'a>(
    modules: &'a [WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
    id: &hir::DeclarationId,
) -> Option<(&'a WorkspaceResolvedModule, &'a hir::ResolvedFunction)> {
    let mut found = None;
    for module in modules {
        for function in &module.functions {
            if function.id != *id {
                continue;
            }
            if found.is_some() {
                return None;
            }
            let fact = facts.get(id.as_str())?;
            if fact.kind != hir::DeclarationKind::Function
                || fact.origin != hir::IdentityOrigin::Explicit
                || fact.owner.is_some()
                || modules
                    .iter()
                    .filter(|owner| owner.path == module.path || owner.module == module.module)
                    .count()
                    != 1
                || fact.path.as_deref() != Some(module.path.as_str())
                || fact.module.as_deref() != Some(module.module.as_str())
            {
                return None;
            }
            found = Some((module, function));
        }
    }
    found
}

// A complete bounded census grants no authority and materializes no scratch
// inventory. Only a potential helper site activates the charged proof walk.
fn candidate(statement: &hir::ResolvedStatement) -> bool {
    matches!(statement, hir::ResolvedStatement::Assign { binding, field: None, value, .. }
        if binding.ty == ResolvedType::Bytes
        && matches!(&value.kind, ResolvedExprKind::Call { callee, args, .. }
            if crate::byte_ops::by_id(callee.as_str()).is_none()
                && args.iter().any(|argument| matches!(&argument.kind, ResolvedExprKind::Place(place)
                    if place.root == binding.id && place.projections.is_empty()))))
}

fn census(function: &hir::ResolvedFunction) -> Option<bool> {
    let mut stack = [None; crate::cache_codec::MAX_DEPTH];
    stack[0] = Some((&function.body, 0usize));
    let mut depth = 1usize;
    let mut work = crate::cache_codec::MAX_NODES;
    let mut found = false;
    while depth != 0 {
        work = work.checked_sub(1)?;
        let index = depth - 1;
        let (expression, next) = stack[index]?;
        if next == 0 {
            if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                work = work.checked_sub(statements.len())?;
                found |= statements.iter().any(candidate);
            }
        }
        stack[index] = Some((expression, next.checked_add(1)?));
        if let Some(child) = hir::lexical_exit::child(expression, next) {
            if depth == stack.len() {
                return None;
            }
            stack[depth] = Some((child, 0));
            depth += 1;
        } else {
            depth -= 1;
        }
    }
    Some(found)
}

#[cfg(test)]
pub(super) fn schema(
    module: &WorkspaceResolvedModule,
    modules: &[WorkspaceResolvedModule],
    facts: &BTreeMap<String, WorkspaceDeclarationFact>,
) -> Result<Option<&'static str>, Vec<Diagnostic>> {
    schema_with_proofs(module, modules, facts, &mut Proofs::new(modules, facts))
}

pub(super) fn schema_with_proofs<'a>(
    module: &'a WorkspaceResolvedModule,
    modules: &'a [WorkspaceResolvedModule],
    facts: &'a BTreeMap<String, WorkspaceDeclarationFact>,
    proofs: &mut Proofs<'a>,
) -> Result<Option<&'static str>, Vec<Diagnostic>> {
    let fail = || {
        vec![graph_error(
            "SPX-G410",
            "retained byte helper renewal authority disagrees",
        )]
    };
    let lookup_work = modules
        .iter()
        .try_fold(0usize, |sum, module| {
            sum.checked_add(module.functions.len())
        })
        .ok_or_else(fail)?;
    let mut any = false;
    for function in module
        .functions
        .iter()
        .chain(module.function_instances.iter().map(|i| &i.function))
    {
        if !census(function).ok_or_else(fail)? {
            if function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19 {
                return Err(fail());
            }
            continue;
        }
        // The caller and every callee must have one source declaration owner.
        let (_, retained) = target(modules, facts, &function.id).ok_or_else(fail)?;
        if !std::ptr::eq(retained, function) {
            return Err(fail());
        }
        let required = function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19;
        let mut found = false;
        let mut stack = [None; crate::cache_codec::MAX_DEPTH];
        reserve_builder_structure(std::mem::size_of_val(&stack))?;
        stack[0] = Some((&function.body, 0usize));
        let mut depth = 1usize;
        let mut work = crate::cache_codec::MAX_NODES;
        while depth != 0 {
            work = work.checked_sub(1).ok_or_else(fail)?;
            let index = depth - 1;
            let (expression, next) = stack[index].ok_or_else(fail)?;
            if next == 0 {
                if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                    work = work.checked_sub(statements.len()).ok_or_else(fail)?;
                    for statement in statements {
                        if !candidate(statement) {
                            continue;
                        }
                        let hir::ResolvedStatement::Assign {
                            binding,
                            field: None,
                            value,
                            ..
                        } = statement
                        else {
                            continue;
                        };
                        if binding.ty != ResolvedType::Bytes {
                            continue;
                        }
                        let ResolvedExprKind::Call {
                            callee,
                            type_arguments,
                            instance,
                            args,
                        } = &value.kind
                        else {
                            continue;
                        };
                        if crate::byte_ops::by_id(callee.as_str()).is_some() {
                            continue;
                        }
                        let Some((_, callee)) = target(modules, facts, callee) else {
                            if required {
                                return Err(fail());
                            }
                            continue;
                        };
                        if instance.is_some() || !type_arguments.is_empty() || value.ty != binding.ty
                            || value.ownership != OwnershipMode::Own || binding.ownership != OwnershipMode::Own
                            || callee.return_type != ResolvedType::Bytes || args.len() != callee.params.len()
                            || callee.params.iter().filter(|p| p.ownership == OwnershipMode::Own).count() != 1
                            || !callee.params.iter().zip(args).all(|(p, a)| p.ty == a.ty && match p.ownership {
                                OwnershipMode::Own => p.ty == ResolvedType::Bytes && a.ownership == OwnershipMode::Own
                                    && matches!(&a.kind, ResolvedExprKind::Place(place) if place.root == binding.id && place.projections.is_empty()),
                                OwnershipMode::Value => a.ownership == OwnershipMode::Value,
                                OwnershipMode::Borrow => matches!(a.ownership, OwnershipMode::Borrow | OwnershipMode::Own),
                                OwnershipMode::Shared => false,
                            }) {
                            if required { return Err(fail()); }
                            continue;
                        }
                        if !proofs.prove(modules, facts, callee, lookup_work)? {
                            // Ordinary allocating helpers retain the old profile.
                            // A v19 claim must replay every candidate exactly.
                            if required {
                                return Err(fail());
                            }
                            continue;
                        }
                        found = true;
                    }
                }
            }
            stack[index] = Some((expression, next.checked_add(1).ok_or_else(fail)?));
            if let Some(child) = hir::lexical_exit::child(expression, next) {
                if depth == stack.len() {
                    return Err(fail());
                }
                stack[depth] = Some((child, 0));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
        if found != (function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19) {
            return Err(fail());
        }
        any |= found;
    }
    Ok(any.then_some("semaprax.graph.v76"))
}
