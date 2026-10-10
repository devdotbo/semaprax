use super::*;
#[derive(Clone, Copy)]
enum Frame<'a> {
    Function(&'a ResolvedFunction, usize),
    Expression(&'a ResolvedFunction, &'a ResolvedExpr, usize),
}
pub(crate) fn nonallocating(program: &ResolvedProgram, target: &ResolvedFunction) -> bool {
    nonallocating_with(
        target,
        |id, instance| program.resolve_call_target(id, instance),
        |_, mode, ty| resolved_param(program, mode, ty),
        |_, ty| {
            crate::hir::is_scalar_resolved_type(ty)
                || crate::hir::copy_record_collection::admitted(&program.declarations, ty)
                || super::super::resolved_match_scrutinee_admitted(&program.declarations, ty)
        },
        program
            .functions
            .len()
            .saturating_add(program.function_instances.len()),
    )
}
pub(crate) fn nonallocating_with<'a>(
    target: &'a ResolvedFunction,
    lookup: impl Fn(
        &crate::hir::DeclarationId,
        Option<&crate::hir::FunctionInstanceId>,
    ) -> Option<&'a ResolvedFunction>,
    param_admit: impl Fn(&ResolvedFunction, OwnershipMode, &ResolvedType) -> bool,
    result_admit: impl Fn(&ResolvedFunction, &ResolvedType) -> bool,
    lookup_work: usize,
) -> bool {
    prove(target, &lookup, &param_admit, &result_admit, lookup_work).is_some()
}
fn prove<'a>(
    target: &'a ResolvedFunction,
    lookup: &impl Fn(
        &crate::hir::DeclarationId,
        Option<&crate::hir::FunctionInstanceId>,
    ) -> Option<&'a ResolvedFunction>,
    param_admit: &impl Fn(&ResolvedFunction, OwnershipMode, &ResolvedType) -> bool,
    result_admit: &impl Fn(&ResolvedFunction, &ResolvedType) -> bool,
    lookup_work: usize,
) -> Option<()> {
    if !crate::bounded_output::reserve_active_required(std::mem::size_of::<
        [Option<Frame<'_>>; DEPTH],
    >()) {
        return None;
    }
    let mut frames = [None; DEPTH];
    frames[0] = Some(Frame::Function(target, 0));
    let mut depth = 1usize;
    let mut work = WORK;
    while depth != 0 {
        work = work.checked_sub(1)?;
        let current = depth - 1;
        let child = match frames[current]? {
            Frame::Function(function, root) => {
                if root == 0
                    && (!function.effects.is_empty()
                        || function.yields.is_some()
                        || !function
                            .params
                            .iter()
                            .all(|p| param_admit(function, p.ownership, &p.ty))
                        || !(function.return_type == ResolvedType::Bytes
                            || result_admit(function, &function.return_type)
                                && function.return_type != ResolvedType::String))
                {
                    return None;
                }
                let expression = if root < function.requires.len() {
                    function.requires.get(root)
                } else if root == function.requires.len() {
                    Some(&function.body)
                } else {
                    function.ensures.get(root - function.requires.len() - 1)
                };
                frames[current] = Some(Frame::Function(function, root + 1));
                expression.map(|e| Frame::Expression(function, e, usize::MAX))
            }
            Frame::Expression(function, expression, next) => {
                if next == usize::MAX {
                    frames[current] = Some(Frame::Expression(function, expression, 0));
                    match &expression.kind {
                        ResolvedExprKind::String(_)
                        | ResolvedExprKind::LiteralFormat { .. }
                        | ResolvedExprKind::Closure { .. }
                        | ResolvedExprKind::Invoke { .. }
                        | ResolvedExprKind::FunctionReference { .. }
                        | ResolvedExprKind::NativeRustImportCall(_)
                        | ResolvedExprKind::HostCommandCall(_)
                        | ResolvedExprKind::Yield { .. }
                        | ResolvedExprKind::Try { .. }
                        | ResolvedExprKind::TryOption { .. }
                        | ResolvedExprKind::LexicalExit { .. } => return None,
                        ResolvedExprKind::ConstructRecord { .. }
                        | ResolvedExprKind::ConstructVariant { .. }
                        | ResolvedExprKind::UpdateRecord { .. } => {
                            if expression.ownership != OwnershipMode::Value
                                || !result_admit(function, &expression.ty)
                            {
                                return None;
                            }
                        }
                        ResolvedExprKind::Block { statements, .. } => {
                            work = work.checked_sub(statements.len())?;
                            if statements
                                .iter()
                                .any(|s| matches!(s, crate::hir::ResolvedStatement::Unsafe { .. }))
                            {
                                return None;
                            }
                        }
                        ResolvedExprKind::Call {
                            callee,
                            type_arguments,
                            instance,
                            args,
                        } => {
                            if let Some(op) = crate::byte_ops::by_id(callee.as_str()) {
                                if !byte_operation(op)
                                    || !type_arguments.is_empty()
                                    || args.len() != op.arity()
                                {
                                    return None;
                                }
                            } else if let Some(op) = crate::string_ops::by_id(callee.as_str()) {
                                if !text_operation(op) {
                                    return None;
                                }
                            } else if let Some(op) = crate::vec_ops::by_id(callee.as_str()) {
                                if !vec_operation(op) {
                                    return None;
                                }
                            } else {
                                work = work.checked_sub(lookup_work.checked_add(depth)?)?;
                                let target = lookup(callee, instance.as_ref())?;
                                if instance.is_some() || !type_arguments.is_empty() || args.len() != target.params.len()
                                    || expression.ty != target.return_type
                                    || args.iter().zip(&target.params).any(|(a, p)| a.ty != p.ty || !match p.ownership {
                                        OwnershipMode::Own => a.ownership == OwnershipMode::Own,
                                        OwnershipMode::Value => a.ownership == OwnershipMode::Value,
                                        OwnershipMode::Borrow => matches!(a.ownership, OwnershipMode::Own | OwnershipMode::Borrow),
                                        OwnershipMode::Shared => false,
                                    })
                                    || frames[..depth].iter().any(|f| matches!(f, Some(Frame::Function(active, _)) if active.id == target.id)) { return None; }
                                if depth == DEPTH {
                                    return None;
                                }
                                frames[depth] = Some(Frame::Function(target, 0));
                                depth += 1;
                                continue;
                            }
                        }
                        _ => {}
                    }
                    continue;
                }
                work = work.checked_sub(next.checked_add(1)?.checked_mul(2)?)?;
                frames[current] = Some(Frame::Expression(function, expression, next + 1));
                crate::hir::lexical_exit::child(expression, next)
                    .map(|e| Frame::Expression(function, e, usize::MAX))
            }
        };
        if let Some(child) = child {
            if depth == DEPTH {
                return None;
            }
            frames[depth] = Some(child);
            depth += 1;
        } else {
            frames[current] = None;
            depth -= 1;
        }
    }
    Some(())
}
pub(crate) fn bindings<'a>(
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
) -> std::collections::BTreeMap<crate::hir::ExpressionId, &'a ResolvedBinding> {
    checked_bindings(program, function).unwrap_or_default()
}

pub(crate) fn checked_bindings<'a>(
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
) -> Result<
    std::collections::BTreeMap<crate::hir::ExpressionId, &'a ResolvedBinding>,
    crate::diagnostic::Diagnostic,
> {
    let error = || {
        crate::diagnostic::Diagnostic::io(
            "SPX-H006",
            "byte helper renewal census exceeds admitted scratch, work or depth",
        )
    };
    let mut found = std::collections::BTreeMap::new();
    // No new census applies to modules without a potentially relevant signature.
    if !program.functions.iter().any(|f| {
        f.return_type == ResolvedType::Bytes
            && f.params
                .iter()
                .any(|p| p.ownership == OwnershipMode::Own && p.ty == ResolvedType::Bytes)
    }) {
        return Ok(found);
    }
    if !crate::bounded_output::reserve_active_required(std::mem::size_of::<
        [(&ResolvedExpr, usize, usize); DEPTH],
    >()) {
        return Err(error());
    }
    let mut frames = [(&function.body, 0usize, 0usize); DEPTH];
    let mut depth = 1usize;
    let mut work = WORK;
    while depth != 0 {
        work = work.checked_sub(1).ok_or_else(error)?;
        let (expression, next, sub) = frames[depth - 1];
        if next == 0 && sub == 0 {
            if let ResolvedExprKind::Block { statements, .. } = &expression.kind {
                work = work.checked_sub(statements.len()).ok_or_else(error)?;
                for statement in statements {
                    if let crate::hir::ResolvedStatement::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } = statement
                    {
                        if assignment(program, binding, value) {
                            found.insert(value.id.clone(), binding);
                        }
                    }
                }
            }
        }
        // Block and Match cursors never repeatedly rescan a growing prefix.
        let child = match &expression.kind {
            ResolvedExprKind::Block { statements, tail } => {
                if let Some(statement) = statements.get(next) {
                    let child = statement.child(sub);
                    if sub + 1 >= statement.child_count() {
                        frames[depth - 1] = (expression, next + 1, 0);
                    } else {
                        frames[depth - 1].2 += 1;
                    }
                    if child.is_none() {
                        continue;
                    }
                    child
                } else {
                    frames[depth - 1].1 += 1;
                    (next == statements.len()).then_some(tail.as_ref())
                }
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                if next == 0 {
                    frames[depth - 1].1 = 1;
                    Some(scrutinee.as_ref())
                } else if let Some(arm) = arms.get(next - 1) {
                    if sub == 0 && arm.guard.is_some() {
                        frames[depth - 1].2 = 1;
                        arm.guard.as_ref()
                    } else {
                        frames[depth - 1] = (expression, next + 1, 0);
                        Some(&arm.value)
                    }
                } else {
                    None
                }
            }
            _ => {
                frames[depth - 1].1 += 1;
                crate::hir::lexical_exit::child(expression, next)
            }
        };
        if let Some(child) = child {
            if depth == DEPTH {
                return Err(error());
            }
            frames[depth] = (child, 0, 0);
            depth += 1;
        } else {
            depth -= 1;
        }
    }
    Ok(found)
}
