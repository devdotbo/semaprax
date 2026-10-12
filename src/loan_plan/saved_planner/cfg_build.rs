//! Saved original CFG construction and `Vec`-returning child collection.

use super::*;

pub(in crate::loan_plan) fn build_cfg<'a>(
    function: &'a ResolvedFunction,
    work: &mut WorkCounter,
) -> Result<Cfg<'a>, Diagnostic> {
    let roots = function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(function.ensures.iter())
        .collect::<Vec<_>>();
    let mut expressions = Vec::new();
    let mut seen = BTreeSet::new();
    let mut root_by_expression = BTreeMap::<ExpressionId, ExpressionId>::new();
    let mut pending = roots
        .iter()
        .rev()
        .map(|root| (*root, *root))
        .collect::<Vec<_>>();
    while let Some((expression, root)) = pending.pop() {
        charge(work)?;
        if !seen.insert(expression.id.clone()) {
            return Err(error("CFG contains a duplicate expression identity"));
        }
        root_by_expression.insert(expression.id.clone(), root.id.clone());
        expressions.push(expression);
        let mut children = Vec::new();
        push_children(expression, &mut children);
        pending.extend(children.into_iter().map(|child| (child, root)));
    }
    let node_count = expressions
        .len()
        .checked_mul(2)
        .ok_or_else(|| error("CFG point count overflows"))?;
    if node_count > MAX_LOAN_ENDPOINTS_V1 {
        return Err(error("function exceeds 4,096 loan program points"));
    }
    let mut points = Vec::with_capacity(node_count);
    let mut nodes = BTreeMap::new();
    for expression in &expressions {
        for phase in [LoanPointPhase::Before, LoanPointPhase::After] {
            let point = point(expression, phase);
            let id =
                u16::try_from(points.len()).map_err(|_| error("CFG point identity overflows"))?;
            nodes.insert(point.clone(), id);
            points.push(point);
        }
    }
    let node = |expression: &ResolvedExpr, phase: LoanPointPhase| {
        nodes
            .get(&point(expression, phase))
            .copied()
            .ok_or_else(|| error("CFG child point is not indexed"))
    };
    let root_after = |expression: &ResolvedExpr| {
        let root = root_by_expression
            .get(&expression.id)
            .ok_or_else(|| error("CFG expression lacks a function-exit root"))?;
        nodes
            .get(&LoanProgramPoint {
                expression: root.clone(),
                phase: LoanPointPhase::After,
            })
            .copied()
            .ok_or_else(|| error("CFG function-exit root is not indexed"))
    };
    let mut edge_set = BTreeSet::<(u16, u16)>::new();
    for expression in &expressions {
        charge(work)?;
        let before = node(expression, LoanPointPhase::Before)?;
        let after = node(expression, LoanPointPhase::After)?;
        match &expression.kind {
            ResolvedExprKind::LexicalExit { kind, value, .. } => {
                if let Some(value) = value {
                    edge_set.insert((before, node(value, LoanPointPhase::Before)?));
                    edge_set.insert((node(value, LoanPointPhase::After)?, after));
                } else {
                    edge_set.insert((before, after));
                }
                let destination = if *kind == crate::ast::LexicalExitKind::Return {
                    root_after(expression)?
                } else {
                    node(
                        lexical_exit::destination(expression, &expressions, work)?,
                        LoanPointPhase::Before,
                    )?
                };
                edge_set.insert((after, destination));
            }
            ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } => {
                let entry = statements
                    .first()
                    .map(statement_entry)
                    .unwrap_or(tail.as_ref());
                edge_set.insert((before, node(entry, LoanPointPhase::Before)?));
                for (index, statement) in statements.iter().enumerate() {
                    let next = statements
                        .get(index + 1)
                        .map(statement_entry)
                        .unwrap_or(tail.as_ref());
                    match statement {
                        ResolvedStatement::While {
                            condition, body, ..
                        }
                        | ResolvedStatement::LexicalWhile {
                            condition, body, ..
                        } => {
                            edge_set.insert((
                                node(condition, LoanPointPhase::After)?,
                                node(body, LoanPointPhase::Before)?,
                            ));
                            edge_set.insert((
                                node(condition, LoanPointPhase::After)?,
                                node(next, LoanPointPhase::Before)?,
                            ));
                            if body.ty != ResolvedType::Never {
                                edge_set.insert((
                                    node(body, LoanPointPhase::After)?,
                                    node(condition, LoanPointPhase::Before)?,
                                ));
                            }
                        }
                        _ => {
                            edge_set.insert((
                                node(statement_exit(statement), LoanPointPhase::After)?,
                                node(next, LoanPointPhase::Before)?,
                            ));
                        }
                    }
                }
                if tail.ty != ResolvedType::Never {
                    edge_set.insert((node(tail, LoanPointPhase::After)?, after));
                }
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                edge_set.insert((before, node(condition, LoanPointPhase::Before)?));
                edge_set.insert((
                    node(condition, LoanPointPhase::After)?,
                    node(then_branch, LoanPointPhase::Before)?,
                ));
                edge_set.insert((
                    node(condition, LoanPointPhase::After)?,
                    node(else_branch, LoanPointPhase::Before)?,
                ));
                if then_branch.ty != ResolvedType::Never {
                    edge_set.insert((node(then_branch, LoanPointPhase::After)?, after));
                }
                if else_branch.ty != ResolvedType::Never {
                    edge_set.insert((node(else_branch, LoanPointPhase::After)?, after));
                }
            }
            ResolvedExprKind::Binary {
                op: BinaryOp::And | BinaryOp::Or,
                left,
                right,
            } => {
                edge_set.insert((before, node(left, LoanPointPhase::Before)?));
                edge_set.insert((
                    node(left, LoanPointPhase::After)?,
                    node(right, LoanPointPhase::Before)?,
                ));
                edge_set.insert((node(left, LoanPointPhase::After)?, after));
                edge_set.insert((node(right, LoanPointPhase::After)?, after));
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                edge_set.insert((before, node(scrutinee, LoanPointPhase::Before)?));
                for (index, arm) in arms.iter().enumerate() {
                    let entry = arm.guard.as_deref().unwrap_or(&arm.value);
                    edge_set.insert((
                        node(scrutinee, LoanPointPhase::After)?,
                        node(entry, LoanPointPhase::Before)?,
                    ));
                    if let Some(guard) = &arm.guard {
                        edge_set.insert((
                            node(guard, LoanPointPhase::After)?,
                            node(&arm.value, LoanPointPhase::Before)?,
                        ));
                        if let Some(next) = arms.get(index + 1) {
                            let next = next.guard.as_deref().unwrap_or(&next.value);
                            edge_set.insert((
                                node(guard, LoanPointPhase::After)?,
                                node(next, LoanPointPhase::Before)?,
                            ));
                        }
                    }
                    if arm.value.ty != ResolvedType::Never {
                        edge_set.insert((node(&arm.value, LoanPointPhase::After)?, after));
                    }
                }
            }
            ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
                let operand_after = node(operand, LoanPointPhase::After)?;
                edge_set.insert((before, node(operand, LoanPointPhase::Before)?));
                edge_set.insert((operand_after, after));
                // `?` has two semantic successors: normal unwrapping and an
                // immediate residual return from this contract/body root.
                // Keeping the residual edge distinct makes a later loan use
                // live only on the normal path and terminates it on return.
                edge_set.insert((operand_after, root_after(expression)?));
            }
            _ => {
                let children = evaluation_children(expression);
                if let Some(first) = children.first() {
                    edge_set.insert((before, node(first, LoanPointPhase::Before)?));
                    for pair in children.windows(2) {
                        edge_set.insert((
                            node(pair[0], LoanPointPhase::After)?,
                            node(pair[1], LoanPointPhase::Before)?,
                        ));
                    }
                    edge_set.insert((
                        node(children.last().expect("nonempty"), LoanPointPhase::After)?,
                        after,
                    ));
                } else {
                    edge_set.insert((before, after));
                }
            }
        }
    }
    let edges = edge_set.into_iter().collect::<Vec<_>>();
    if edges.len() > MAX_LOAN_EDGES_V1 {
        return Err(error("function exceeds 4,096 loan CFG edges"));
    }
    let mut successors = vec![Vec::new(); points.len()];
    let mut successor_edges = vec![Vec::new(); points.len()];
    let mut predecessors = vec![Vec::new(); points.len()];
    for (edge, (from, to)) in edges.iter().enumerate() {
        successors[*from as usize].push(*to);
        successor_edges[*from as usize].push(edge as u16);
        predecessors[*to as usize].push(*from);
    }
    Ok(Cfg {
        points,
        nodes,
        expressions,
        edges,
        successors,
        successor_edges,
        predecessors,
    })
}

pub(in crate::loan_plan) fn evaluation_children(expression: &ResolvedExpr) -> Vec<&ResolvedExpr> {
    match &expression.kind {
        ResolvedExprKind::LexicalExit { value, .. } => value.as_deref().into_iter().collect(),
        ResolvedExprKind::Closure { captures, .. } => {
            captures.iter().map(|capture| &capture.value).collect()
        }
        ResolvedExprKind::FunctionReference { .. } => Vec::new(),
        ResolvedExprKind::Invoke { callable, args } => std::iter::once(callable.as_ref())
            .chain(args.iter())
            .collect(),
        ResolvedExprKind::Call { args, .. }
        | ResolvedExprKind::LiteralFormat { args, .. }
        | ResolvedExprKind::VecFieldRead { args, .. }
        | ResolvedExprKind::RowLoan { args, .. } => args.iter().collect(),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.iter().collect(),
        ResolvedExprKind::HostCommandCall(call) => call.args.iter().collect(),
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => vec![source, start, end],
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Yield { request: value } => vec![value],
        ResolvedExprKind::Binary { left, right, .. } => vec![left, right],
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            fields.iter().map(|field| &field.value).collect()
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => std::iter::once(base.as_ref())
            .chain(fields.iter().map(|field| &field.value))
            .collect(),
        ResolvedExprKind::Block { .. }
        | ResolvedExprKind::VecConstructor { .. }
        | ResolvedExprKind::If { .. }
        | ResolvedExprKind::Match { .. }
        | ResolvedExprKind::Int(_)
        | ResolvedExprKind::Int32(_)
        | ResolvedExprKind::Char(_)
        | ResolvedExprKind::Uint8(_)
        | ResolvedExprKind::Usize(_)
        | ResolvedExprKind::ArrayU8(_)
        | ResolvedExprKind::RepeatArrayU8 { .. }
        | ResolvedExprKind::Float32(_)
        | ResolvedExprKind::Float64(_)
        | ResolvedExprKind::Bool(_)
        | ResolvedExprKind::String(_)
        | ResolvedExprKind::BorrowedUtf8Literal(_)
        | ResolvedExprKind::Place(_)
        | ResolvedExprKind::BorrowPlace { .. } => Vec::new(),
    }
}
