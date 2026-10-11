//! Independent Copy-inert region discovery from authenticated HIR.
//! A borrowed owned receiver is admitted by its parameter mode, never by
//! rewriting its Own Place expression into a Copy expression. Unknown call
//! families, ownership transfers, loops, residuals and lexical exits stay closed.
use super::*;

fn debit(
    budget: &mut ReplayBudget,
    function: &ResolvedFunction,
    units: usize,
    phase: &str,
) -> Result<(), Diagnostic> {
    if budget.merge_paths {
        budget.charge_skeleton(function, units, phase)
    } else {
        budget.charge(function, units, phase)
    }
}

fn copy_type(ty: &ResolvedType) -> bool {
    crate::hir::is_scalar_resolved_type(ty)
        || matches!(
            ty,
            ResolvedType::Unit
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::ArrayU8(_)
        )
}

fn borrowed_place(expression: &ResolvedExpr) -> bool {
    matches!(
        expression.kind,
        ResolvedExprKind::Place(_) | ResolvedExprKind::BorrowPlace { .. }
    ) && matches!(
        expression.ownership,
        OwnershipMode::Own | OwnershipMode::Borrow | OwnershipMode::Value
    )
}

fn call_mode(
    program: &dyn ReplayContext,
    expression: &ResolvedExpr,
    index: usize,
) -> Option<OwnershipMode> {
    let ResolvedExprKind::Call {
        callee,
        instance,
        type_arguments,
        args,
    } = &expression.kind
    else {
        return None;
    };
    if let Some(op) = crate::vec_ops::by_id(callee.as_str()).filter(|_| instance.is_none()) {
        if !matches!(
            op,
            crate::vec_ops::VecOp::Len
                | crate::vec_ops::VecOp::Capacity
                | crate::vec_ops::VecOp::Get
        ) || args.len() != op.arity()
        {
            return None;
        }
        let [element] = type_arguments.as_slice() else {
            return None;
        };
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = &args.first()?.ty
        else {
            return None;
        };
        if declaration.as_str() != crate::prelude::VEC_ID
            || arguments.as_slice() != std::slice::from_ref(element)
        {
            return None;
        }
        let result = match op {
            crate::vec_ops::VecOp::Len | crate::vec_ops::VecOp::Capacity => &ResolvedType::Usize,
            crate::vec_ops::VecOp::Get => element,
            _ => return None,
        };
        if result != &expression.ty || (args.len() == 2 && args[1].ty != ResolvedType::Usize) {
            return None;
        }
        return (index < args.len()).then(|| op.param_ownership(index));
    }
    let target = program.resolve_call_target(callee, instance.as_ref())?;
    if target.return_type != expression.ty || target.params.len() != args.len() {
        return None;
    }
    let parameter = target.params.get(index)?;
    if parameter.ty != args[index].ty {
        return None;
    }
    Some(parameter.ownership)
}

/// Inspect this exact borrowed HIR region without cloning its types or body.
/// The verdict carries no path-merge or strategy authority by itself.
pub(in crate::cleanup_plan::replay) fn eligible(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    root: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    const DEPTH: usize = 514;
    budget.reserve_array::<Option<expression_children::ReplayChildCursor<'_>>>(
        function,
        DEPTH,
        "Copy region discovery cursor",
    )?;
    let mut frames = [None; DEPTH];
    let mut length = 1;
    frames[0] = Some(expression_children::ReplayChildCursor::new(root));
    while length != 0 {
        debit(budget, function, 1, "Copy region discovery step")?;
        let frame = frames[length - 1]
            .as_mut()
            .expect("Copy discovery frame retained");
        let expression = frame.expression();
        if !copy_type(&expression.ty) {
            return Ok(false);
        }
        match &expression.kind {
            ResolvedExprKind::Int(_)
            | ResolvedExprKind::Int32(_)
            | ResolvedExprKind::Char(_)
            | ResolvedExprKind::Uint8(_)
            | ResolvedExprKind::Usize(_)
            | ResolvedExprKind::ArrayU8(_)
            | ResolvedExprKind::Float32(_)
            | ResolvedExprKind::Float64(_)
            | ResolvedExprKind::Bool(_)
            | ResolvedExprKind::BorrowedUtf8Literal(_)
            | ResolvedExprKind::Place(_)
            | ResolvedExprKind::BorrowPlace { .. }
            | ResolvedExprKind::Unary { .. }
            | ResolvedExprKind::Binary { .. }
            | ResolvedExprKind::If { .. } => {}
            ResolvedExprKind::Block { statements, .. } => {
                for statement in statements {
                    debit(budget, function, 1, "Copy region statement mode")?;
                    match statement {
                        ResolvedStatement::Let { binding, .. }
                        | ResolvedStatement::Assign { binding, .. }
                            if copy_type(&binding.ty)
                                && binding.ownership != OwnershipMode::Own => {}
                        _ => return Ok(false),
                    }
                }
            }
            ResolvedExprKind::Call { args, .. } => {
                for (index, argument) in args.iter().enumerate() {
                    debit(
                        budget,
                        function,
                        1,
                        "Copy region authenticated parameter mode",
                    )?;
                    match call_mode(program, expression, index) {
                        Some(OwnershipMode::Borrow) if borrowed_place(argument) => {}
                        Some(OwnershipMode::Value) if copy_type(&argument.ty) => {}
                        _ => return Ok(false),
                    }
                }
                // Even a zero-argument call must bind its exact result target.
                if args.is_empty() {
                    let ResolvedExprKind::Call {
                        callee, instance, ..
                    } = &expression.kind
                    else {
                        unreachable!()
                    };
                    if !program
                        .resolve_call_target(callee, instance.as_ref())
                        .is_some_and(|target| {
                            target.params.is_empty() && target.return_type == expression.ty
                        })
                    {
                        return Ok(false);
                    }
                }
            }
            _ => return Ok(false),
        }
        match frame.next() {
            Some(child) => {
                if let ResolvedExprKind::Call { args, .. } = &expression.kind {
                    let index = args
                        .iter()
                        .position(|argument| std::ptr::eq(argument, child))
                        .expect("call child is an exact argument");
                    if call_mode(program, expression, index) == Some(OwnershipMode::Borrow)
                        && borrowed_place(child)
                    {
                        continue;
                    }
                }
                if length == DEPTH {
                    return Err(replay_error(
                        function,
                        "Copy region discovery exceeds the admitted expression depth",
                    ));
                }
                frames[length] = Some(expression_children::ReplayChildCursor::new(child));
                length += 1;
            }
            None => {
                frames[length - 1] = None;
                length -= 1;
            }
        }
    }
    Ok(true)
}

#[cfg(test)]
mod tests;
