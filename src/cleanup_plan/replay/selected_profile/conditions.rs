//! Rebuild the existing String condition-only read set under the receiving
//! invocation's physical and work ledger. No attached index is consulted.
use super::*;
type Frame<'a> = Option<(crate::hir::lexical_exit::ChildCursor<'a>, bool)>;
const DEPTH: usize = 514;
pub(in crate::cleanup_plan::replay) fn reads(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<BTreeSet<ExpressionId>, Diagnostic> {
    budget.reserve_array::<Frame<'_>>(function, DEPTH, "selected String condition outer cursor")?;
    budget.reserve_array::<Frame<'_>>(function, DEPTH, "selected String condition inner cursor")?;
    let mut result = BTreeSet::new();
    let mut frames = [None; DEPTH];
    frames[0] = Some((
        crate::hir::lexical_exit::ChildCursor::new(&function.body),
        false,
    ));
    let mut depth = 1;
    while depth != 0 {
        budget.charge(function, 1, "selected String condition outer node")?;
        let (mut cursor, entered) = frames[depth - 1].expect("current frame");
        let expression = cursor.expression();
        if !entered {
            if let ResolvedExprKind::Block { statements, .. }
            | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
            {
                budget.charge(
                    function,
                    statements.len(),
                    "selected String condition statement census",
                )?;
                for statement in statements {
                    // Match the ordinary function_reads authority exactly. Lexical
                    // loop conditions retain their separately admitted cloning path.
                    if let crate::hir::ResolvedStatement::While { condition, .. } = statement {
                        condition_reads(function, condition, &mut result, budget)?;
                    }
                }
            }
            frames[depth - 1] = Some((cursor, true));
            continue;
        }
        if let Some(child) = cursor.next() {
            if depth == DEPTH {
                return Err(replay_error(
                    function,
                    "selected String condition depth refused",
                ));
            }
            frames[depth - 1] = Some((cursor, true));
            frames[depth] = Some((crate::hir::lexical_exit::ChildCursor::new(child), false));
            depth += 1;
        } else {
            depth -= 1
        }
    }
    Ok(result)
}
fn condition_reads(
    function: &ResolvedFunction,
    root: &ResolvedExpr,
    result: &mut BTreeSet<ExpressionId>,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let mut frames = [None; DEPTH];
    frames[0] = Some((crate::hir::lexical_exit::ChildCursor::new(root), false));
    let mut depth = 1;
    while depth != 0 {
        budget.charge(function, 1, "selected String condition inner node")?;
        let (mut cursor, entered) = frames[depth - 1].expect("current frame");
        let expression = cursor.expression();
        if !entered {
            if let Some(arguments) = crate::string_ops::conditions::resolved_operands(expression) {
                for argument in arguments {
                    let units = (result.len() + 1)
                        .checked_mul(argument.id.as_str().len() + 1)
                        .and_then(|n| n.checked_mul(2))
                        .ok_or_else(|| {
                            replay_error(function, "selected String read key work overflowed")
                        })?;
                    budget.charge(
                        function,
                        units,
                        "selected String condition key comparison and copy",
                    )?;
                    if !result.contains(&argument.id) {
                        budget.reserve_btree_node::<ExpressionId, ()>(
                            function,
                            "selected String read set",
                        )?;
                        budget.reserve_clone(
                            function,
                            &argument.id,
                            "selected String read identity",
                        )?;
                        result.insert(argument.id.clone());
                    }
                }
                depth -= 1;
                continue;
            }
            frames[depth - 1] = Some((cursor, true));
            continue;
        }
        if let Some(child) = cursor.next() {
            // Legacy condition_reads traverses a nested ordinary While's
            // condition, never its body. All other child shapes stay authored.
            let skip = matches!(expression.kind, ResolvedExprKind::Block { .. })
                && matches!(cursor.last_statement_child(),
                Some((crate::hir::ResolvedStatement::While { body, .. }, _))
                    if std::ptr::eq(body.as_ref(), child));
            frames[depth - 1] = Some((cursor, true));
            if skip {
                continue;
            }
            if depth == DEPTH {
                return Err(replay_error(
                    function,
                    "selected String condition depth refused",
                ));
            }
            frames[depth] = Some((crate::hir::lexical_exit::ChildCursor::new(child), false));
            depth += 1;
        } else {
            depth -= 1
        }
    }
    Ok(())
}

pub(in crate::cleanup_plan::replay) fn moves(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<BTreeSet<ExpressionId>, Diagnostic> {
    budget.reserve_array::<Frame<'_>>(function, DEPTH, "selected String move cursor")?;
    let mut result = BTreeSet::new();
    let mut frames = [None; DEPTH];
    frames[0] = Some((
        crate::hir::lexical_exit::ChildCursor::new(&function.body),
        false,
    ));
    let mut depth = 1;
    while depth != 0 {
        budget.charge(function, 1, "selected String move node")?;
        let (mut cursor, entered) = frames[depth - 1].expect("current frame");
        let expression = cursor.expression();
        if !entered {
            if let ResolvedExprKind::Block { statements, .. }
            | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
            {
                budget.charge(
                    function,
                    statements.len(),
                    "selected String move statements",
                )?;
                for statement in statements {
                    if let crate::hir::ResolvedStatement::Assign {
                        binding,
                        value,
                        field: None,
                        ..
                    } = statement
                    {
                        if binding.ty == ResolvedType::String
                            && super::collections::append(function, binding, value, budget)?
                        {
                            let ResolvedExprKind::Call { args, .. } = &value.kind else {
                                unreachable!()
                            };
                            let id = &args[0].id;
                            budget.charge(
                                function,
                                (result.len() + 1)
                                    .checked_mul(id.as_str().len() + 1)
                                    .and_then(|n| n.checked_mul(2))
                                    .ok_or_else(|| {
                                        replay_error(function, "selected move key work overflow")
                                    })?,
                                "selected String move key",
                            )?;
                            if !result.contains(id) {
                                budget.reserve_btree_node::<ExpressionId, ()>(
                                    function,
                                    "selected String move set",
                                )?;
                                budget.reserve_clone(
                                    function,
                                    id,
                                    "selected String moving identity",
                                )?;
                                result.insert(id.clone());
                            }
                        }
                    }
                }
            }
            frames[depth - 1] = Some((cursor, true));
            continue;
        }
        if let Some(child) = cursor.next() {
            if depth == DEPTH {
                return Err(replay_error(function, "selected move depth refused"));
            }
            frames[depth - 1] = Some((cursor, true));
            frames[depth] = Some((crate::hir::lexical_exit::ChildCursor::new(child), false));
            depth += 1;
        } else {
            depth -= 1;
        }
    }
    Ok(result)
}
