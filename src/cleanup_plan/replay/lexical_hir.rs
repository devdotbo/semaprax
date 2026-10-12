//! Independent typed-HIR facts for lexical cleanup replay.
//!
//! These facts come only from resolved HIR and the HIR lexical validators. No
//! certificate, plan block, edge or exit is consulted, so a forged plan can
//! neither add nor hide an exit, a loop or an abrupt (no-normal-completion)
//! flow. Control-flow-dead code after an abrupt statement carries no fact,
//! exactly as it carries no lowering and no replay path.
use super::*;
use crate::ast::LexicalExitKind;
use crate::hir::{ControlTarget, LoopLatchKind, SourceForKind};

/// One reachable lexical `return`, `break` or `continue`, in evaluation order.
pub(super) struct ExitFact<'a> {
    pub(super) expression: &'a ResolvedExpr,
    pub(super) kind: LexicalExitKind,
    pub(super) target: &'a ControlTarget,
    pub(super) value: Option<&'a ResolvedExpr>,
}

/// One reachable lexical loop, in completion (post-) order.
pub(super) struct LoopFact<'a> {
    pub(super) condition: &'a ResolvedExpr,
    pub(super) body: &'a ResolvedExpr,
    pub(super) latch: &'a LoopLatchKind,
    /// `false` when the independently replayed body has no normal endpoint.
    pub(super) body_completes: bool,
}

pub(super) struct LexicalFacts<'a> {
    pub(super) exits: Vec<ExitFact<'a>>,
    pub(super) loops: Vec<LoopFact<'a>>,
    /// `false` when the function body has no normal completion.
    pub(super) body_completes: bool,
    /// Reachable postfix `?` nodes; each can end a terminal residual path.
    pub(super) tries: usize,
    /// Condition identity of every reachable loop (ordinary or lexical) in
    /// lowering (pre-)order. Each owns one per-iteration execution boundary.
    pub(super) boundaries: Vec<&'a ExpressionId>,
    pub(super) requires_boundaries: Vec<&'a ExpressionId>,
    pub(super) ensures_boundaries: Vec<&'a ExpressionId>,
    true_match: Option<&'a ResolvedExpr>,
}

/// Maximum nesting this scan admits; the HIR lexical validator bounds the same
/// resolved tree, so a deeper tree fails closed here rather than recursing.
const MAX_SCAN_DEPTH: usize = 512;

pub(super) fn collect<'a>(
    function: &'a ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<LexicalFacts<'a>, Diagnostic> {
    let mut facts = LexicalFacts {
        exits: Vec::new(),
        loops: Vec::new(),
        body_completes: true,
        tries: 0,
        boundaries: Vec::new(),
        requires_boundaries: Vec::new(),
        ensures_boundaries: Vec::new(),
        true_match: None,
    };
    for clause in &function.requires {
        let boundaries = contract_boundaries(function, clause, budget)?;
        budget.reserve_vec(
            function,
            &mut facts.requires_boundaries,
            boundaries.len(),
            "required contract boundaries",
        )?;
        facts.requires_boundaries.extend(boundaries);
    }
    for clause in &function.ensures {
        let boundaries = contract_boundaries(function, clause, budget)?;
        budget.reserve_vec(
            function,
            &mut facts.ensures_boundaries,
            boundaries.len(),
            "ensured contract boundaries",
        )?;
        facts.ensures_boundaries.extend(boundaries);
    }
    facts.body_completes = scan(function, &function.body, &mut facts, 0, budget)?;
    Ok(facts)
}

// Contracts retain ordinary loop semantics, but never admit lexical exits or
// generated lexical latches across the contract evaluation boundary.
fn contract_boundaries<'a>(
    function: &'a ResolvedFunction,
    clause: &'a ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<Vec<&'a ExpressionId>, Diagnostic> {
    let mut facts = LexicalFacts {
        exits: Vec::new(),
        loops: Vec::new(),
        body_completes: true,
        tries: 0,
        boundaries: Vec::new(),
        requires_boundaries: Vec::new(),
        ensures_boundaries: Vec::new(),
        true_match: None,
    };
    let completes = scan(function, clause, &mut facts, 0, budget)?;
    if !completes || !facts.exits.is_empty() || !facts.loops.is_empty() || facts.tries != 0 {
        return Err(replay_error(
            function,
            "contract crosses the statement lexical-control boundary",
        ));
    }
    Ok(facts.boundaries)
}

/// Evaluate `expression` in authored order. Returns whether it can complete
/// normally. Statements after an abrupt statement are dead and never scanned.
fn scan<'a>(
    function: &'a ResolvedFunction,
    expression: &'a ResolvedExpr,
    facts: &mut LexicalFacts<'a>,
    depth: usize,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    if depth > MAX_SCAN_DEPTH {
        return Err(replay_error(
            function,
            "lexical flow scan exceeds the admitted expression depth",
        ));
    }
    if budget.selected() {
        budget.charge(function, 1, "selected lexical HIR facts")?;
    }
    match &expression.kind {
        ResolvedExprKind::LexicalExit {
            kind,
            target,
            value,
        } => {
            if let Some(operand) = value {
                if !scan(function, operand, facts, depth + 1, budget)? {
                    return Err(replay_error(
                        function,
                        "lexical exit operand has no normal completion",
                    ));
                }
            }
            budget.reserve_vec(function, &mut facts.exits, 1, "lexical exit facts")?;
            facts.exits.push(ExitFact {
                expression,
                kind: *kind,
                target,
                value: value.as_deref(),
            });
            Ok(false)
        }
        ResolvedExprKind::Block { statements, tail }
        | ResolvedExprKind::VecConstructor {
            statements, tail, ..
        } => {
            for statement in statements {
                let completes = match statement {
                    ResolvedStatement::Let { value, .. }
                    | ResolvedStatement::Assign { value, .. }
                    | ResolvedStatement::CopyDiscard { value, .. } => {
                        scan(function, value, facts, depth + 1, budget)?
                    }
                    ResolvedStatement::Unsafe { body, .. } => {
                        scan(function, body, facts, depth + 1, budget)?
                    }
                    ResolvedStatement::While {
                        condition, body, ..
                    } => {
                        let entered = scan(function, condition, facts, depth + 1, budget)?;
                        budget.reserve_vec(
                            function,
                            &mut facts.boundaries,
                            1,
                            "lexical loop boundaries",
                        )?;
                        facts.boundaries.push(&condition.id);
                        scan(function, body, facts, depth + 1, budget)?;
                        entered
                    }
                    ResolvedStatement::LexicalWhile {
                        condition,
                        body,
                        latch,
                        ..
                    } => {
                        let entered = scan(function, condition, facts, depth + 1, budget)?;
                        budget.reserve_vec(
                            function,
                            &mut facts.boundaries,
                            1,
                            "lexical loop boundaries",
                        )?;
                        facts.boundaries.push(&condition.id);
                        crate::hir::lexical_exit::validate_latch(condition, body, latch)?;
                        let previous = facts.true_match;
                        facts.true_match =
                            crate::hir::iterator_loop::true_body_match(condition, body, latch);
                        let body_result = scan(function, body, facts, depth + 1, budget);
                        facts.true_match = previous;
                        let body_completes = body_result?;
                        budget.reserve_vec(function, &mut facts.loops, 1, "lexical loop facts")?;
                        facts.loops.push(LoopFact {
                            condition,
                            body,
                            latch,
                            body_completes,
                        });
                        entered
                    }
                };
                if !completes {
                    return Ok(false);
                }
            }
            scan(function, tail, facts, depth + 1, budget)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            if !scan(function, condition, facts, depth + 1, budget)? {
                return Ok(false);
            }
            let then_completes = scan(function, then_branch, facts, depth + 1, budget)?;
            let else_completes = scan(function, else_branch, facts, depth + 1, budget)?;
            Ok(then_completes || else_completes)
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            if !scan(function, scrutinee, facts, depth + 1, budget)? {
                return Ok(false);
            }
            let mut completes = false;
            let yield_only = facts
                .true_match
                .is_some_and(|value| std::ptr::eq(value, expression));
            for (index, arm) in arms.iter().enumerate() {
                if let Some(guard) = &arm.guard {
                    scan(function, guard, facts, depth + 1, budget)?;
                }
                // Scan BOTH original arms. Only completion is conditioned on
                // the independently proved true-loop predicate, never validity.
                let arm_completes = scan(function, &arm.value, facts, depth + 1, budget)?;
                completes |= arm_completes && (!yield_only || index == 1);
            }
            Ok(completes)
        }
        _ => {
            if matches!(
                expression.kind,
                ResolvedExprKind::Try { .. } | ResolvedExprKind::TryOption { .. }
            ) {
                facts.tries += 1;
            }
            let mut index = 0;
            while let Some(child) = replay_expression_child(expression, index) {
                if !scan(function, child, facts, depth + 1, budget)? {
                    return Ok(false);
                }
                index += 1;
            }
            Ok(true)
        }
    }
}

/// The generated advancement a `continue` must run before re-entering its
/// loop, derived from the authored latch alone: the counter assignment of a
/// source-for Copy/Borrow wrapper, or the Yield-arm tail of an owned iterator.
/// `None` for an ordinary While condition. The latch is independently replayed
/// by `hir::lexical_exit::validate_latch`; the descriptor identities are only
/// compared against the structure found here.
pub(super) fn generated_advance<'a>(
    body: &'a ResolvedExpr,
    latch: &LoopLatchKind,
) -> Option<&'a ResolvedExpr> {
    match latch {
        LoopLatchKind::Condition => None,
        LoopLatchKind::SourceForStep { kind, step } => {
            if *kind == SourceForKind::OwnedIterator {
                return None;
            }
            let ResolvedExprKind::Block { statements, .. } = &body.kind else {
                return None;
            };
            let [_, _, ResolvedStatement::Assign { value, .. }] = statements.as_slice() else {
                return None;
            };
            (&value.id == step).then_some(value)
        }
        LoopLatchKind::OwnedIteratorStep { next, .. } => {
            let ResolvedExprKind::Block { statements, .. } = &body.kind else {
                return None;
            };
            let [ResolvedStatement::Assign { value, .. }] = statements.as_slice() else {
                return None;
            };
            let ResolvedExprKind::Match { arms, .. } = &value.kind else {
                return None;
            };
            let ResolvedExprKind::Block { tail, .. } = &arms.get(1)?.value.kind else {
                return None;
            };
            (&tail.id == next).then_some(tail.as_ref())
        }
    }
}

/// The assignment (its value, then its binding) that renews an owned
/// iterator step with the Yield-arm tail.
pub(super) fn owned_step_assignment(
    body: &ResolvedExpr,
) -> Option<(&ResolvedExpr, &crate::hir::ResolvedBinding)> {
    let ResolvedExprKind::Block { statements, .. } = &body.kind else {
        return None;
    };
    let [ResolvedStatement::Assign { binding, value, .. }] = statements.as_slice() else {
        return None;
    };
    Some((value, binding))
}

/// The owned item binding of an owning traversal's Yield arm, if any. It is
/// disposed (not retained) when a `continue` advances the iterator.
pub(super) fn yield_item(body: &ResolvedExpr) -> Option<&crate::hir::ResolvedBinding> {
    let ResolvedExprKind::Block { statements, .. } = &body.kind else {
        return None;
    };
    let [ResolvedStatement::Assign { value, .. }] = statements.as_slice() else {
        return None;
    };
    let ResolvedExprKind::Match { arms, .. } = &value.kind else {
        return None;
    };
    let ResolvedMatchPattern::Variant { fields, .. } = &arms.get(1)?.pattern else {
        return None;
    };
    fields.first().map(|field| &field.binding)
}

/// The Yield arm block and the match whose arm it is.
pub(super) fn yield_arm(body: &ResolvedExpr) -> Option<(&ResolvedExpr, &ResolvedExpr)> {
    let ResolvedExprKind::Block { statements, .. } = &body.kind else {
        return None;
    };
    let [ResolvedStatement::Assign { value, .. }] = statements.as_slice() else {
        return None;
    };
    let ResolvedExprKind::Match { arms, .. } = &value.kind else {
        return None;
    };
    Some((value, &arms.get(1)?.value))
}
