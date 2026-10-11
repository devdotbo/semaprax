//! Sound path and work bounds for the lexical typed-skeleton comparison.
//!
//! Lexical-free subtrees reuse the legacy census exactly. Only the four
//! lexical-bearing node kinds (block, `if`, `match`, exit) are counted here,
//! with target-qualified breaks and a returning class: a `return` path leaves
//! its contour, then runs every
//! postcondition, so it multiplies by the postcondition paths like a normal
//! body path. Loops are counted once, never by iteration: the proof CFG
//! linearizes one finite iteration and ends it at the latch.
use super::path_summary::path_budget_errors;
use super::*;

#[derive(Clone, Copy, Default)]
struct Counts {
    normal: usize,
    failed: usize,
    returned: usize,
    // Breaks name exactly the nearest borrowed loop context. That loop alone
    // consumes them into ordinary continuation paths.
    broken: usize,
}

impl Counts {
    const ONE: Self = Self {
        normal: 1,
        failed: 0,
        returned: 0,
        broken: 0,
    };

    fn total(self) -> usize {
        self.normal
            .saturating_add(self.failed)
            .saturating_add(self.returned)
            .saturating_add(self.broken)
    }

    fn capped(self) -> Self {
        if self.total() > MAX_REPLAY_PATHS {
            Self {
                normal: MAX_REPLAY_PATHS + 1,
                failed: 0,
                returned: 0,
                broken: 0,
            }
        } else {
            self
        }
    }
}

fn sequence(left: Counts, right: Counts) -> Counts {
    Counts {
        normal: left.normal.saturating_mul(right.normal),
        failed: left
            .failed
            .saturating_add(left.normal.saturating_mul(right.failed)),
        returned: left
            .returned
            .saturating_add(left.normal.saturating_mul(right.returned)),
        broken: left
            .broken
            .saturating_add(left.normal.saturating_mul(right.broken)),
    }
    .capped()
}

/// Marker/continuation headroom per exit; generated advancement work is
/// independently counted from its actual HIR rather than a synthetic path count.
const CONTINUE_WORK: usize = 160;
/// Work per lexical loop statement beyond its condition and body: two
/// sequencings, the Boolean split, the latch observation and the join, each
/// charged once per path, with headroom for the four continuation pushes.
const LOOP_STATEMENT_WORK: usize = 40;

#[derive(Clone, Copy)]
struct Nearest<'a> {
    condition: &'a ResolvedExpr,
    body: &'a ResolvedExpr,
    latch: &'a crate::hir::LoopLatchKind,
}

fn count(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    weight: &mut usize,
    depth: usize,
    nearest: Option<Nearest<'_>>,
) -> Result<Counts, Diagnostic> {
    if depth > 512 {
        return Err(replay_error(
            function,
            "lexical path census exceeds the admitted expression depth",
        ));
    }
    if !crate::hir::lexical_exit::contains(expression) {
        let free = expression_path_counts(function, expression)?;
        let counts = Counts {
            normal: free.normal,
            failed: free.failed,
            // A residual `?` path reaches the shared epilogue and runs every
            // postcondition, exactly like a returning path.
            returned: free.residual,
            broken: 0,
        };
        if counts.total() > MAX_REPLAY_PATHS {
            path_budget_errors(function, 0, counts.total())?;
        }
        *weight = checked_skeleton_add(
            function,
            *weight,
            expression_skeleton_work_upper(program, function, expression)?,
        )?;
        return Ok(counts);
    }
    let mut local: usize;
    let counts = match &expression.kind {
        ResolvedExprKind::LexicalExit {
            kind,
            target,
            value,
        } => {
            local = CONTINUE_WORK;
            let owner = match target {
                crate::hir::ControlTarget::Function(owner)
                | crate::hir::ControlTarget::Loop { owner, .. } => owner,
            };
            crate::hir::lexical_exit::validate_owner(function, owner)?;
            match (kind, target, value) {
                (
                    crate::ast::LexicalExitKind::Return,
                    crate::hir::ControlTarget::Function(_),
                    Some(operand),
                ) => {
                    let operand = count(program, function, operand, weight, depth + 1, nearest)?;
                    Counts {
                        normal: 0,
                        failed: operand.failed,
                        returned: operand.normal.saturating_add(operand.returned),
                        broken: operand.broken,
                    }
                }
                (
                    crate::ast::LexicalExitKind::Break | crate::ast::LexicalExitKind::Continue,
                    crate::hir::ControlTarget::Loop { site, .. },
                    None,
                ) => {
                    let active = nearest
                        .filter(|active| site == &active.condition.id)
                        .ok_or_else(|| {
                            replay_error(
                                function,
                                "lexical census exit differs from its exact nearest loop",
                            )
                        })?;
                    if *kind == crate::ast::LexicalExitKind::Break {
                        Counts {
                            broken: 1,
                            ..Counts::default()
                        }
                    } else {
                        // A continue stops this iteration. Its real generated
                        // advancement may fail, but never enters the suffix.
                        let advance = match super::lexical_hir::generated_advance(
                            active.body,
                            active.latch,
                        ) {
                            Some(advance) => {
                                count(program, function, advance, weight, depth + 1, nearest)?
                            }
                            None if *active.latch == crate::hir::LoopLatchKind::Condition => {
                                Counts::ONE
                            }
                            None => {
                                return Err(replay_error(
                                    function,
                                    "lexical census cannot reconstruct continue advancement",
                                ))
                            }
                        };
                        if advance.returned != 0 || advance.broken != 0 {
                            return Err(replay_error(
                                function,
                                "lexical census advancement crosses its loop boundary",
                            ));
                        }
                        Counts {
                            failed: advance.failed.saturating_add(advance.normal),
                            ..Counts::default()
                        }
                    }
                }
                _ => {
                    return Err(replay_error(
                        function,
                        "lexical census exit kind, target or operand is inconsistent",
                    ))
                }
            }
        }
        ResolvedExprKind::Block { statements, tail }
        | ResolvedExprKind::VecConstructor {
            statements, tail, ..
        } => {
            local = checked_skeleton_add(
                function,
                checked_skeleton_mul(function, statements.len(), 5)?,
                7,
            )?;
            let mut accumulator = Counts::ONE;
            for statement in statements {
                match statement {
                    ResolvedStatement::Let { binding, value, .. }
                    | ResolvedStatement::Assign { binding, value, .. } => {
                        if binding.ownership == OwnershipMode::Own
                            && type_needs_drop(program, function, &binding.ty)?
                        {
                            local = checked_skeleton_add(function, local, 6)?;
                        }
                        let child = count(program, function, value, weight, depth + 1, nearest)?;
                        accumulator = sequence(accumulator, child);
                    }
                    ResolvedStatement::Unsafe { body, .. } => {
                        let child = count(program, function, body, weight, depth + 1, nearest)?;
                        accumulator = sequence(accumulator, child);
                    }
                    ResolvedStatement::While {
                        condition, body, ..
                    } => {
                        if crate::hir::lexical_exit::contains(body) {
                            return Err(replay_error(
                                function,
                                "legacy while cannot contain lexical exit authority",
                            ));
                        }
                        local = checked_skeleton_add(function, local, 6)?;
                        let condition =
                            count(program, function, condition, weight, depth + 1, nearest)?;
                        let body = count(program, function, body, weight, depth + 1, nearest)?;
                        accumulator = sequence(
                            accumulator,
                            Counts {
                                normal: condition
                                    .normal
                                    .saturating_mul(body.normal.saturating_add(1)),
                                failed: condition
                                    .failed
                                    .saturating_add(condition.normal.saturating_mul(body.failed)),
                                returned: condition
                                    .returned
                                    .saturating_add(condition.normal.saturating_mul(body.returned)),
                                broken: condition
                                    .broken
                                    .saturating_add(condition.normal.saturating_mul(body.broken)),
                            },
                        );
                    }
                    ResolvedStatement::LexicalWhile {
                        condition,
                        body,
                        latch,
                        ..
                    } => {
                        local = checked_skeleton_add(function, local, LOOP_STATEMENT_WORK)?;
                        crate::hir::lexical_exit::validate_latch(condition, body, latch)?;
                        let body_nearest = Some(Nearest {
                            condition,
                            body,
                            latch,
                        });
                        let condition =
                            count(program, function, condition, weight, depth + 1, nearest)?;
                        let body = count(program, function, body, weight, depth + 1, body_nearest)?;
                        accumulator = sequence(
                            accumulator,
                            Counts {
                                normal: condition
                                    .normal
                                    .saturating_mul(body.broken.saturating_add(1)),
                                failed: condition.failed.saturating_add(
                                    condition
                                        .normal
                                        .saturating_mul(body.failed.saturating_add(body.normal)),
                                ),
                                returned: condition
                                    .returned
                                    .saturating_add(condition.normal.saturating_mul(body.returned)),
                                broken: condition.broken,
                            }
                            .capped(),
                        );
                    }
                }
            }
            if expression.ownership == OwnershipMode::Own
                && type_needs_drop(program, function, &expression.ty)?
            {
                local = checked_skeleton_add(function, local, 6)?;
            }
            let tail = count(program, function, tail, weight, depth + 1, nearest)?;
            sequence(accumulator, tail)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            local = 10;
            let condition = count(program, function, condition, weight, depth + 1, nearest)?;
            let then_branch = count(program, function, then_branch, weight, depth + 1, nearest)?;
            let else_branch = count(program, function, else_branch, weight, depth + 1, nearest)?;
            Counts {
                normal: condition
                    .normal
                    .saturating_mul(then_branch.normal.saturating_add(else_branch.normal)),
                failed: condition.failed.saturating_add(
                    condition
                        .normal
                        .saturating_mul(then_branch.failed.saturating_add(else_branch.failed)),
                ),
                returned: condition.returned.saturating_add(
                    condition
                        .normal
                        .saturating_mul(then_branch.returned.saturating_add(else_branch.returned)),
                ),
                broken: condition.broken.saturating_add(
                    condition
                        .normal
                        .saturating_mul(then_branch.broken.saturating_add(else_branch.broken)),
                ),
            }
            .capped()
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            local = arms.len().saturating_mul(32).saturating_add(8);
            let scrutinee = count(program, function, scrutinee, weight, depth + 1, nearest)?;
            let mut arm_normal = 0usize;
            let mut arm_failed = 0usize;
            let mut arm_returned = 0usize;
            let mut arm_broken = 0usize;
            let true_match = nearest.is_some_and(|active| {
                crate::hir::iterator_loop::true_body_match(
                    active.condition,
                    active.body,
                    active.latch,
                )
                .is_some_and(|current| std::ptr::eq(current, expression))
            });
            for (index, arm) in arms.iter().enumerate() {
                let alternatives = arm
                    .pattern
                    .case_iter()
                    .map_or(1, |cases| cases.len())
                    .max(1);
                let (guard_total, guard_failed) = match &arm.guard {
                    Some(guard) => {
                        let guard = count(program, function, guard, weight, depth + 1, nearest)?;
                        (guard.total(), guard.failed)
                    }
                    None => (0, 0),
                };
                let value = count(program, function, &arm.value, weight, depth + 1, nearest)?;
                // The recognizer authenticates the childless Done constructor
                // and both predicate leaves; still scan its full authored HIR
                // above. Only runtime paths under the true condition omit it.
                if true_match && index == 0 {
                    continue;
                }
                let multiplicity = alternatives.saturating_mul(guard_total.saturating_add(1));
                arm_normal = arm_normal.saturating_add(value.normal.saturating_mul(multiplicity));
                arm_failed = arm_failed
                    .saturating_add(value.failed.saturating_mul(multiplicity))
                    .saturating_add(guard_failed.saturating_mul(alternatives));
                arm_returned =
                    arm_returned.saturating_add(value.returned.saturating_mul(multiplicity));
                arm_broken = arm_broken.saturating_add(value.broken.saturating_mul(multiplicity));
            }
            Counts {
                normal: scrutinee.normal.saturating_mul(arm_normal),
                failed: scrutinee
                    .failed
                    .saturating_add(scrutinee.normal.saturating_mul(arm_failed)),
                returned: scrutinee
                    .returned
                    .saturating_add(scrutinee.normal.saturating_mul(arm_returned)),
                broken: scrutinee
                    .broken
                    .saturating_add(scrutinee.normal.saturating_mul(arm_broken)),
            }
            .capped()
        }
        _ => {
            return Err(replay_error(
                function,
                "lexical exit appears in a position outside the statement-shaped profile",
            ));
        }
    };
    let paths = counts.total().max(1);
    if paths > MAX_REPLAY_PATHS {
        path_budget_errors(function, 0, paths)?;
    }
    *weight = checked_skeleton_add(
        function,
        *weight,
        checked_skeleton_mul(function, local, paths)?,
    )?;
    Ok(counts)
}

/// Terminal semantic paths and HIR skeleton work of one lexical function.
fn bounds(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) -> Result<(usize, usize), Diagnostic> {
    let mut weight = 0usize;
    let mut paths = Counts::ONE;
    for contract in &function.requires {
        let child = count(program, function, contract, &mut weight, 0, None)?;
        paths = sequence(paths, child);
        paths.failed = paths.failed.saturating_add(paths.normal);
    }
    let body = count(program, function, &function.body, &mut weight, 0, None)?;
    paths = sequence(paths, body);
    if paths.broken != 0 {
        return Err(replay_error(
            function,
            "lexical census break escaped its exact loop",
        ));
    }
    // A returning path runs every postcondition exactly like a normal one.
    paths = Counts {
        normal: paths.normal.saturating_add(paths.returned),
        failed: paths.failed,
        returned: 0,
        broken: paths.broken,
    }
    .capped();
    for contract in &function.ensures {
        let child = count(program, function, contract, &mut weight, 0, None)?;
        paths = sequence(paths, child);
        paths.failed = paths.failed.saturating_add(paths.normal);
        if paths.broken != 0 {
            return Err(replay_error(
                function,
                "lexical census break escaped its exact loop",
            ));
        }
    }
    Ok((paths.total(), weight))
}

pub(super) fn validate_size_budget(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    validate_size_budget_with_expression_count(program, function, || {
        Ok(expression_facts(function)?.len())
    })
}

/// Construct and authenticate a selected fact owner at exactly the legacy
/// census stage, then let its caller retain and reuse it for complete replay.
pub(super) fn validate_size_budget_with_expression_count(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    count: impl FnOnce() -> Result<usize, Diagnostic>,
) -> Result<(), Diagnostic> {
    validate_census(program, function, count, false).map(|_| ())
}

/// Reuse the selected census backing and semantic walk for its skeleton
/// reservation; constructing a second CFG census would allocate again.
pub(super) fn validate_size_budget_and_skeleton(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    count: impl FnOnce() -> Result<usize, Diagnostic>,
) -> Result<usize, Diagnostic> {
    validate_census(program, function, count, true)
}

fn validate_census(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    count: impl FnOnce() -> Result<usize, Diagnostic>,
    skeleton: bool,
) -> Result<usize, Diagnostic> {
    let structure_units = plan_structure_units(&function.cleanup_plan);
    if structure_units > MAX_REPLAY_WORK_UNITS {
        return Err(replay_error(
            function,
            "cleanup replay structure exceeds the global work budget",
        ));
    }
    let cfg = branch_sensitive_cfg_bounds(function)?;
    let (semantic_paths, hir_weight) = bounds(program, function)?;
    path_budget_errors(function, cfg.terminal_paths, semantic_paths)?;
    let expression_units = count()?;
    if cfg.work.saturating_add(expression_units) > MAX_REPLAY_WORK_UNITS {
        return Err(replay_error(
            function,
            "cleanup replay combined path/work bound exceeds the global budget",
        ));
    }
    if skeleton {
        skeleton_from_bounds(program, function, semantic_paths, hir_weight, || Ok(cfg))
    } else {
        Ok(0)
    }
}

/// Program-wide reservation for one lexical function's skeleton comparison.
/// Mirrors the legacy formula, with the lexical path census above.
pub(super) fn skeleton_work_upper(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) -> Result<usize, Diagnostic> {
    let (semantic_paths, hir_weight) = bounds(program, function)?;
    skeleton_from_bounds(program, function, semantic_paths, hir_weight, || {
        branch_sensitive_cfg_bounds(function)
    })
}

fn skeleton_from_bounds(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    semantic_paths: usize,
    hir_weight: usize,
    cfg: impl FnOnce() -> Result<CfgReplayBounds, Diagnostic>,
) -> Result<usize, Diagnostic> {
    let semantic_paths = semantic_paths.max(1);
    if semantic_paths > MAX_REPLAY_PATHS {
        path_budget_errors(function, 0, semantic_paths)?;
    }
    // Root path, four body-sequencing operations and completed path; an owned
    // result adds its identity clone and four transfers; a returning path also
    // marks its exit and is reclassified once.
    let root_work = if type_needs_drop(program, function, &function.return_type)? {
        14
    } else {
        9
    };
    let mut hir_upper = checked_skeleton_mul(function, root_work, semantic_paths)?;
    hir_upper = checked_skeleton_add(function, hir_upper, hir_weight)?;
    hir_upper = checked_skeleton_add(
        function,
        hir_upper,
        checked_skeleton_mul(
            function,
            function
                .requires
                .len()
                .checked_add(function.ensures.len())
                .and_then(|contracts| contracts.checked_mul(6))
                .ok_or_else(|| skeleton_preflight_overflow(function))?,
            semantic_paths,
        )?,
    )?;
    let cfg = cfg()?;
    if cfg.terminal_paths > MAX_REPLAY_PATHS {
        path_budget_errors(function, cfg.terminal_paths, 0)?;
    }
    let plan_terminals = checked_skeleton_mul(function, cfg.terminal_paths.max(1), 4)?;
    let comparison = checked_skeleton_mul(function, semantic_paths.max(cfg.terminal_paths), 2)?;
    checked_skeleton_add(
        function,
        checked_skeleton_add(function, hir_upper, cfg.skeleton_work)?,
        checked_skeleton_add(function, plan_terminals, comparison)?,
    )
}

#[cfg(test)]
#[path = "lexical_census/tests.rs"]
mod tests;
