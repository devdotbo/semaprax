//! The first physically reserved retained lexical profile is deliberately
//! primitive/direct-flat-record and module-local. Refusal happens before delegated nested nominal,
//! dynamic provider, replacement, format or iterator constructors allocate.
use super::*;
const DEPTH: usize = 514;
type Frame<'a> = Option<(crate::hir::lexical_exit::ChildCursor<'a>, bool)>;
pub(super) fn validate(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if budget.physical.is_none() {
        return Ok(());
    }
    flat_records::module(program, function, budget)?;
    if !program.interfaces().is_empty()
        || !program.function_templates().is_empty()
        || !program.function_instances().is_empty()
        || !function.effects.is_empty()
        || function.yields.is_some()
        || !flat_records::admitted(program, &function.return_type)?
    {
        return Err(replay_error(
            function,
            "selected lexical replay requires a module-local primitive context",
        ));
    }
    for parameter in &function.params {
        if !flat_records::admitted(program, &parameter.ty)? {
            return Err(replay_error(
                function,
                "selected parameter type is outside direct records",
            ));
        }
    }
    budget.reserve_array::<Frame<'_>>(function, DEPTH, "selected primitive profile cursor")?;
    let mut frames = [None; DEPTH];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        frames[0] = Some((crate::hir::lexical_exit::ChildCursor::new(root), false));
        let mut depth = 1;
        while depth != 0 {
            budget.charge(function, 1, "selected primitive profile node")?;
            let (cursor, entered) = frames[depth - 1]
                .ok_or_else(|| replay_error(function, "selected profile cursor is empty"))?;
            let expression = cursor.expression();
            if !entered {
                if expression.ty != ResolvedType::Never
                    && !flat_records::admitted(program, &expression.ty)?
                {
                    return Err(replay_error(function, "selected lexical replay does not admit nominal or callable expression storage"));
                }
                flat_records::expression(program, function, expression)?;
                match &expression.kind {
                    ResolvedExprKind::Place(_) if expression.ty == ResolvedType::String => {
                        return Err(replay_error(function, "selected lexical replay String-place condition indexes are not reserved"));
                    }
                    ResolvedExprKind::ConstructVariant { .. }
                    | ResolvedExprKind::UpdateRecord { .. }
                    | ResolvedExprKind::Upcast { .. }
                    | ResolvedExprKind::LiteralFormat { .. }
                    | ResolvedExprKind::Try { .. }
                    | ResolvedExprKind::TryOption { .. }
                    | ResolvedExprKind::Closure { .. }
                    | ResolvedExprKind::Invoke { .. }
                    | ResolvedExprKind::FunctionReference { .. }
                    | ResolvedExprKind::NativeRustImportCall(_)
                    | ResolvedExprKind::HostCommandCall(_)
                    | ResolvedExprKind::VecFieldRead { .. }
                    | ResolvedExprKind::Yield { .. } => {
                        return Err(replay_error(function, "selected lexical replay provider is outside the reserved primitive profile"));
                    }
                    ResolvedExprKind::BorrowedUtf8Literal(value) => {
                        if expression.ty != ResolvedType::Str
                            || expression.ownership != OwnershipMode::Borrow
                            || !value.is_backed()
                        {
                            return Err(replay_error(
                                function,
                                "selected static literal has invalid type, ownership or backing",
                            ));
                        }
                        crate::hir::static_text::validate_literal(value)?;
                    }
                    ResolvedExprKind::Block { statements, .. } => {
                        budget.charge(
                            function,
                            statements.len(),
                            "selected primitive statements",
                        )?;
                        for statement in statements {
                            match statement {
                                crate::hir::ResolvedStatement::Let { binding, .. }
                                | crate::hir::ResolvedStatement::Assign { binding, .. }
                                    if !flat_records::admitted(program, &binding.ty)? =>
                                {
                                    return Err(replay_error(
                                        function,
                                        "selected block binding requires unsupported storage",
                                    ));
                                }
                                crate::hir::ResolvedStatement::Assign {
                                    field: Some(_), ..
                                } => {
                                    return Err(replay_error(function, "selected primitive assignment cannot target a nominal field"));
                                }
                                crate::hir::ResolvedStatement::LexicalWhile { latch, .. }
                                    if *latch != crate::hir::LoopLatchKind::Condition =>
                                {
                                    return Err(replay_error(function, "selected lexical replay source-for latch is outside the reserved primitive profile"));
                                }
                                crate::hir::ResolvedStatement::Assign { binding, .. }
                                    if binding.ty == ResolvedType::String
                                        || matches!(binding.ty, ResolvedType::Nominal { .. }) =>
                                {
                                    return Err(replay_error(function, "selected lexical replay String replacement is outside the reserved primitive profile"));
                                }
                                crate::hir::ResolvedStatement::Unsafe { .. } => {
                                    return Err(replay_error(
                                        function,
                                        "selected lexical replay has an unsafe statement",
                                    ));
                                }
                                _ => {}
                            }
                        }
                    }
                    ResolvedExprKind::Match {
                        scrutinee, arms, ..
                    } => {
                        if matches!(scrutinee.ty, ResolvedType::Nominal { .. }) {
                            return Err(replay_error(function, "selected direct records do not admit owning or destructuring matches"));
                        }
                        for arm in arms {
                            validate_pattern(function, &arm.pattern, budget, 0)?;
                        }
                    }
                    ResolvedExprKind::Call {
                        callee,
                        instance,
                        type_arguments,
                        ..
                    } => {
                        if instance.is_some()
                            || !type_arguments.is_empty()
                            || crate::string_ops::by_id(callee.as_str())
                                .is_some_and(|op| matches!(op, crate::string_ops::StringOp::Concat))
                        {
                            return Err(replay_error(
                                function,
                                "selected lexical replay call needs an unsupported provider",
                            ));
                        }
                        if crate::byte_ops::by_id(callee.as_str()).is_none()
                            && crate::string_ops::by_id(callee.as_str()).is_none()
                            && crate::str_ops::by_id(callee.as_str()).is_none()
                        {
                            budget.charge(
                                function,
                                program.functions().len(),
                                "selected current callee lookup",
                            )?;
                            if program.resolve_call_target(callee, None).is_none() {
                                return Err(replay_error(
                                    function,
                                    "selected lexical replay call has no current primitive target",
                                ));
                            }
                        }
                    }
                    _ => {}
                }
                frames[depth - 1] = Some((cursor, true));
                continue;
            }
            // The entry census prepays block statement boundaries. Saved
            // cursor progress makes each advance (including None) constant.
            let mut cursor = cursor;
            if let Some(child) = cursor.next() {
                if depth == DEPTH {
                    return Err(replay_error(
                        function,
                        "selected primitive profile exceeds ordinary HIR depth",
                    ));
                }
                frames[depth - 1] = Some((cursor, true));
                frames[depth] = Some((crate::hir::lexical_exit::ChildCursor::new(child), false));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
    }
    Ok(())
}

// Match bindings remain on the frozen primitive pattern boundary. Direct
// records are admitted as ordinary construction/calls/projection, not patterns.
fn primitive(ty: &ResolvedType) -> bool {
    crate::hir::is_scalar_resolved_type(ty)
        || matches!(
            ty,
            ResolvedType::Bytes
                | ResolvedType::String
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::ArrayU8(_)
        )
}

fn validate_pattern(
    function: &ResolvedFunction,
    pattern: &ResolvedMatchPattern,
    budget: &mut ReplayBudget,
    depth: usize,
) -> Result<(), Diagnostic> {
    budget.charge(function, 1, "selected primitive match pattern")?;
    if depth > 256 {
        return Err(replay_error(
            function,
            "selected match pattern exceeds depth",
        ));
    }
    match pattern {
        ResolvedMatchPattern::Wildcard | ResolvedMatchPattern::Literal(_) => Ok(()),
        ResolvedMatchPattern::Binding(binding) if primitive(&binding.ty) => Ok(()),
        ResolvedMatchPattern::Or(alternatives) => {
            for pattern in alternatives {
                validate_pattern(function, pattern, budget, depth + 1)?;
            }
            Ok(())
        }
        ResolvedMatchPattern::Binding(_)
        | ResolvedMatchPattern::Variant { .. }
        | ResolvedMatchPattern::Record { .. } => Err(replay_error(
            function,
            "selected match pattern requires unsupported storage",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn checked() -> crate::hir::ResolvedProgram {
        let parsed = crate::parse(
            "module app; @id(\"app.main\") fn main()->i64 {if true {0} else {1}}",
            "selected-profile-cursor.spx",
        )
        .unwrap();
        crate::hir::resolve(&parsed).unwrap()
    }
    fn budget(limit: usize) -> ReplayBudget {
        let mut budget = ReplayBudget::new();
        budget.physical = Some(Box::new(
            super::super::physical_storage::PhysicalReservation::new(limit),
        ));
        budget
    }
    #[test]
    fn child_cursor_frames_reserve_exact_and_one_short_before_profile_scan() {
        let program = checked();
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let bytes = std::mem::size_of::<[Frame<'_>; DEPTH]>();
        let mut exact = budget(bytes);
        validate(&program, function, &mut exact).unwrap();
        assert_eq!(exact.physical.as_ref().unwrap().charged(), bytes);
        let mut short = budget(bytes - 1);
        assert_eq!(
            validate(&program, function, &mut short).unwrap_err().code,
            "SPX-H006"
        );
        assert!(short.physical.as_mut().unwrap().reserve(0).is_err());
    }
    #[test]
    fn child_cursor_checks_the_late_branch_without_granting_hostile_nominal_storage() {
        let program = checked();
        let mut function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap()
            .clone();
        let ResolvedExprKind::Block { tail, .. } = &mut function.body.kind else {
            panic!("body block")
        };
        let ResolvedExprKind::If { else_branch, .. } = &mut tail.kind else {
            panic!("if tail")
        };
        else_branch.ty = ResolvedType::Nominal {
            declaration: crate::hir::DeclarationId::new("forged.record"),
            arguments: Vec::new(),
        };
        let error = validate(&program, &function, &mut budget(usize::MAX)).unwrap_err();
        assert_eq!(error.code, "SPX-H006");
        assert!(error
            .message
            .contains("unauthenticated or unsupported fields"));
    }
}

#[cfg(test)]
mod cursor_work_tests {
    use super::*;
    fn program(width: usize) -> crate::hir::ResolvedProgram {
        let statements = (0..width)
            .map(|i| format!("let n{i}=0;"))
            .collect::<String>();
        let source = format!("module app; @id(\"app.main\") fn main()->i64 {{{statements}0}}");
        crate::hir::resolve(&crate::parse(&source, "linear-profile.spx").unwrap()).unwrap()
    }
    fn run(program: &crate::hir::ResolvedProgram, limit: usize) -> Result<usize, Diagnostic> {
        let mut budget = ReplayBudget::new();
        budget.remaining = limit;
        budget.physical = Some(Box::new(
            super::super::physical_storage::PhysicalReservation::new(usize::MAX),
        ));
        validate(program, &program.functions[0], &mut budget)?;
        Ok(budget.remaining)
    }
    #[test]
    fn wide_statement_census_and_terminal_advances_have_exact_linear_work() {
        for width in [0, 1, 64, 512] {
            let program = program(width);
            // Block+tail+initializers: entry and terminal probes, each edge,
            // and exactly one statement census; no prefix re-scan term.
            let exact = 4 * width + 5;
            assert_eq!(run(&program, exact).unwrap(), 0);
            assert_eq!(run(&program, exact - 1).unwrap_err().code, "SPX-H006");
        }
    }
    #[test]
    fn guarded_match_patterns_and_all_contract_roots_keep_their_work_debits() {
        let mut matched = program(0);
        let ResolvedExprKind::Block { tail, .. } = &mut matched.functions[0].body.kind else {
            panic!()
        };
        let seed = tail.clone();
        let arms = (0..64)
            .map(|i| crate::hir::ResolvedMatchArm {
                pattern: ResolvedMatchPattern::Wildcard,
                guard: (i % 2 == 0).then(|| {
                    let mut guard = seed.clone();
                    guard.kind = ResolvedExprKind::Bool(true);
                    guard.ty = ResolvedType::Bool;
                    guard
                }),
                value: seed.as_ref().clone(),
                span: seed.span,
            })
            .collect();
        tail.kind = ResolvedExprKind::Match {
            scrutinee: seed,
            arms,
            mode: crate::hir::ResolvedMatchMode::Value,
        };
        let exact = 8 + 4 * 64 + 3 * 32;
        assert_eq!(run(&matched, exact).unwrap(), 0);
        assert!(run(&matched, exact - 1).is_err());
        // Separate roots share the same global ReplayBudget; no fresh permit
        // appears at requires/body/ensures transitions.
        let source = "module app; @id(\"app.main\") fn main()->i64 requires true ensures true {0}";
        let contracts =
            crate::hir::resolve(&crate::parse(source, "contract-roots.spx").unwrap()).unwrap();
        assert_eq!(run(&contracts, 9).unwrap(), 0);
        assert!(run(&contracts, 8).is_err());
    }
    #[test]
    fn deep_cursor_has_exact_work_and_retains_depth_and_late_type_refusal() {
        let mut deep = program(0);
        let ResolvedExprKind::Block { tail, .. } = &mut deep.functions[0].body.kind else {
            panic!()
        };
        for _ in 0..32 {
            let inner = tail.clone();
            tail.kind = ResolvedExprKind::Unary {
                op: crate::ast::UnaryOp::Neg,
                value: inner,
            };
        }
        assert_eq!(run(&deep, 5 + 3 * 32).unwrap(), 0);
        assert!(run(&deep, 5 + 3 * 32 - 1).is_err());
        let ResolvedExprKind::Block { tail, .. } = &mut deep.functions[0].body.kind else {
            panic!()
        };
        for _ in 32..DEPTH {
            let inner = tail.clone();
            tail.kind = ResolvedExprKind::Unary {
                op: crate::ast::UnaryOp::Neg,
                value: inner,
            };
        }
        assert!(run(&deep, usize::MAX).is_err(), "work cannot buy depth");
        let mut late = program(64);
        let ResolvedExprKind::Block { tail, .. } = &mut late.functions[0].body.kind else {
            panic!()
        };
        tail.ty = ResolvedType::Nominal {
            declaration: crate::hir::DeclarationId::new("forged"),
            arguments: Vec::new(),
        };
        assert!(
            run(&late, usize::MAX).is_err(),
            "late unsupported storage remains checked"
        );
    }
}
