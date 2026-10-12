//! Identical CFG construction/order with explicit caller-paid actual storage.
use super::*;

pub(super) fn build<'a>(
    function: &'a ResolvedFunction,
    work: &mut WorkCounter,
) -> Result<Cfg<'a>, Diagnostic> {
    let mut roots = Vec::new();
    storage::extend(&mut roots, function.requires.iter().chain(std::iter::once(&function.body))
        .chain(function.ensures.iter()), work.allocation)?;
    let mut expressions = Vec::new();
    let mut seen = BTreeSet::new();
    let mut root_by_expression = BTreeMap::<ExpressionId, ExpressionId>::new();
    let mut pending = Vec::new();
    storage::extend(&mut pending, roots.iter().rev().map(|root| (*root, *root)),
        work.allocation)?;
    while let Some((expression, root)) = pending.pop() {
        charge(work)?;
        if !storage::set_insert(&mut seen, expression.id.clone(), work.allocation)? {
            return Err(error("CFG contains a duplicate expression identity"));
        }
        storage::map_insert(&mut root_by_expression, expression.id.clone(), root.id.clone(), work.allocation)?;
        storage::push(&mut expressions, expression, work.allocation)?;
        let mut children = Vec::new();
        paid_children::push_children(expression, &mut children, work.allocation)?;
        storage::extend(&mut pending, children.into_iter().map(|child| (child, root)), work.allocation)?;
    }
    let node_count = expressions
        .len()
        .checked_mul(2)
        .ok_or_else(|| error("CFG point count overflows"))?;
    if node_count > MAX_LOAN_ENDPOINTS_V1 {
        return Err(error("function exceeds 4,096 loan program points"));
    }
    let mut points = storage::vector(node_count, work.allocation)?;
    let mut nodes = BTreeMap::new();
    for expression in &expressions {
        for phase in [LoanPointPhase::Before, LoanPointPhase::After] {
            let point = point(expression, phase);
            let id =
                u16::try_from(points.len()).map_err(|_| error("CFG point identity overflows"))?;
            storage::map_insert(&mut nodes, point.clone(), id, work.allocation)?;
            storage::push(&mut points, point, work.allocation)?;
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
                    storage::set_insert(&mut edge_set, (before, node(value, LoanPointPhase::Before)?), work.allocation)?;
                    storage::set_insert(&mut edge_set, (node(value, LoanPointPhase::After)?, after), work.allocation)?;
                } else {
                    storage::set_insert(&mut edge_set, (before, after), work.allocation)?;
                }
                let destination = if *kind == crate::ast::LexicalExitKind::Return {
                    root_after(expression)?
                } else {
                    node(
                        lexical_exit::destination(expression, &expressions, work)?,
                        LoanPointPhase::Before,
                    )?
                };
                storage::set_insert(&mut edge_set, (after, destination), work.allocation)?;
            }
            ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } => {
                let entry = statements
                    .first()
                    .map(statement_entry)
                    .unwrap_or(tail.as_ref());
                storage::set_insert(&mut edge_set, (before, node(entry, LoanPointPhase::Before)?), work.allocation)?;
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
                            storage::set_insert(&mut edge_set, (
                                node(condition, LoanPointPhase::After)?,
                                node(body, LoanPointPhase::Before)?,
                            ), work.allocation)?;
                            storage::set_insert(&mut edge_set, (
                                node(condition, LoanPointPhase::After)?,
                                node(next, LoanPointPhase::Before)?,
                            ), work.allocation)?;
                            if body.ty != ResolvedType::Never {
                                storage::set_insert(&mut edge_set, (
                                    node(body, LoanPointPhase::After)?,
                                    node(condition, LoanPointPhase::Before)?,
                                ), work.allocation)?;
                            }
                        }
                        _ => {
                            storage::set_insert(&mut edge_set, (
                                node(statement_exit(statement), LoanPointPhase::After)?,
                                node(next, LoanPointPhase::Before)?,
                            ), work.allocation)?;
                        }
                    }
                }
                if tail.ty != ResolvedType::Never {
                    storage::set_insert(&mut edge_set, (node(tail, LoanPointPhase::After)?, after), work.allocation)?;
                }
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                storage::set_insert(&mut edge_set, (before, node(condition, LoanPointPhase::Before)?), work.allocation)?;
                storage::set_insert(&mut edge_set, (
                    node(condition, LoanPointPhase::After)?,
                    node(then_branch, LoanPointPhase::Before)?,
                ), work.allocation)?;
                storage::set_insert(&mut edge_set, (
                    node(condition, LoanPointPhase::After)?,
                    node(else_branch, LoanPointPhase::Before)?,
                ), work.allocation)?;
                if then_branch.ty != ResolvedType::Never {
                    storage::set_insert(&mut edge_set, (node(then_branch, LoanPointPhase::After)?, after), work.allocation)?;
                }
                if else_branch.ty != ResolvedType::Never {
                    storage::set_insert(&mut edge_set, (node(else_branch, LoanPointPhase::After)?, after), work.allocation)?;
                }
            }
            ResolvedExprKind::Binary {
                op: BinaryOp::And | BinaryOp::Or,
                left,
                right,
            } => {
                storage::set_insert(&mut edge_set, (before, node(left, LoanPointPhase::Before)?), work.allocation)?;
                storage::set_insert(&mut edge_set, (
                    node(left, LoanPointPhase::After)?,
                    node(right, LoanPointPhase::Before)?,
                ), work.allocation)?;
                storage::set_insert(&mut edge_set, (node(left, LoanPointPhase::After)?, after), work.allocation)?;
                storage::set_insert(&mut edge_set, (node(right, LoanPointPhase::After)?, after), work.allocation)?;
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                storage::set_insert(&mut edge_set, (before, node(scrutinee, LoanPointPhase::Before)?), work.allocation)?;
                for (index, arm) in arms.iter().enumerate() {
                    let entry = arm.guard.as_deref().unwrap_or(&arm.value);
                    storage::set_insert(&mut edge_set, (
                        node(scrutinee, LoanPointPhase::After)?,
                        node(entry, LoanPointPhase::Before)?,
                    ), work.allocation)?;
                    if let Some(guard) = &arm.guard {
                        storage::set_insert(&mut edge_set, (
                            node(guard, LoanPointPhase::After)?,
                            node(&arm.value, LoanPointPhase::Before)?,
                        ), work.allocation)?;
                        if let Some(next) = arms.get(index + 1) {
                            let next = next.guard.as_deref().unwrap_or(&next.value);
                            storage::set_insert(&mut edge_set, (
                                node(guard, LoanPointPhase::After)?,
                                node(next, LoanPointPhase::Before)?,
                            ), work.allocation)?;
                        }
                    }
                    if arm.value.ty != ResolvedType::Never {
                        storage::set_insert(&mut edge_set, (node(&arm.value, LoanPointPhase::After)?, after), work.allocation)?;
                    }
                }
            }
            ResolvedExprKind::Try { operand, .. } | ResolvedExprKind::TryOption { operand, .. } => {
                let operand_after = node(operand, LoanPointPhase::After)?;
                storage::set_insert(&mut edge_set, (before, node(operand, LoanPointPhase::Before)?), work.allocation)?;
                storage::set_insert(&mut edge_set, (operand_after, after), work.allocation)?;
                // `?` has two semantic successors: normal unwrapping and an
                // immediate residual return from this contract/body root.
                // Keeping the residual edge distinct makes a later loan use
                // live only on the normal path and terminates it on return.
                storage::set_insert(&mut edge_set, (operand_after, root_after(expression)?), work.allocation)?;
            }
            _ => {
                let mut children = evaluation_children(expression);
                if let Some(first) = children.next() {
                    storage::set_insert(&mut edge_set, (before, node(first, LoanPointPhase::Before)?), work.allocation)?;
                    let mut last = first;
                    for next in children {
                        storage::set_insert(&mut edge_set, (
                            node(last, LoanPointPhase::After)?,
                            node(next, LoanPointPhase::Before)?,
                        ), work.allocation)?;
                        last = next;
                    }
                    storage::set_insert(&mut edge_set, (node(last, LoanPointPhase::After)?, after), work.allocation)?;
                } else {
                    storage::set_insert(&mut edge_set, (before, after), work.allocation)?;
                }
            }
        }
    }
    let mut edges = storage::vector(edge_set.len(), work.allocation)?;
    edges.extend(edge_set);
    if edges.len() > MAX_LOAN_EDGES_V1 {
        return Err(error("function exceeds 4,096 loan CFG edges"));
    }
    let adjacency = adjacency::Adjacency::build_with_storage(points.len(), &edges, work.allocation)?;
    Ok(Cfg {
        points,
        nodes,
        expressions,
        edges,
        adjacency,
    })
}
