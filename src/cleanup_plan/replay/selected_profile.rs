//! The first physically reserved retained lexical profile is deliberately
//! primitive and module-local. Refusal happens before delegated nominal,
//! dynamic provider, replacement, format or iterator constructors allocate.
use super::*;
const DEPTH: usize = 514;
type Frame<'a> = Option<(crate::hir::lexical_exit::ChildCursor<'a>, bool)>;
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
pub(super) fn validate(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if budget.physical.is_none() {
        return Ok(());
    }
    if !program.types().is_empty()
        || !program.interfaces().is_empty()
        || !program.function_templates().is_empty()
        || !program.function_instances().is_empty()
        || !function.effects.is_empty()
        || function.yields.is_some()
        || !primitive(&function.return_type)
        || function.params.iter().any(|p| !primitive(&p.ty))
    {
        return Err(replay_error(
            function,
            "selected lexical replay requires a module-local primitive context",
        ));
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
                if !primitive(&expression.ty) && expression.ty != ResolvedType::Never {
                    return Err(replay_error(function, "selected lexical replay does not admit nominal or callable expression storage"));
                }
                match &expression.kind {
                    ResolvedExprKind::Place(_) if expression.ty == ResolvedType::String => {
                        return Err(replay_error(function, "selected lexical replay String-place condition indexes are not reserved"));
                    }
                    ResolvedExprKind::ConstructRecord { .. }
                    | ResolvedExprKind::ConstructVariant { .. }
                    | ResolvedExprKind::Project { .. }
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
                                    if !primitive(&binding.ty) =>
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
                                    if binding.ty == ResolvedType::String =>
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
                    ResolvedExprKind::Match { arms, .. } => {
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
            let width = match &expression.kind {
                ResolvedExprKind::Block { statements, .. } => statements.len(),
                ResolvedExprKind::Match { arms, .. } => arms.len(),
                _ => 0,
            };
            budget.charge(function, width, "selected primitive child lookup")?;
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
            .contains("nominal or callable expression storage"));
    }
}
