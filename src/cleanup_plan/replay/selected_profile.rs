//! The first physically reserved retained lexical profile is deliberately
//! primitive and module-local. Refusal happens before delegated nominal,
//! dynamic provider, replacement, format or iterator constructors allocate.
use super::*;
const DEPTH: usize = 514;
type Frame<'a> = Option<(&'a ResolvedExpr, usize)>;
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
        frames[0] = Some((root, usize::MAX));
        let mut depth = 1;
        while depth != 0 {
            budget.charge(function, 1, "selected primitive profile node")?;
            let (expression, next) = frames[depth - 1]
                .ok_or_else(|| replay_error(function, "selected profile cursor is empty"))?;
            if next == usize::MAX {
                if !primitive(&expression.ty) && expression.ty != ResolvedType::Never {
                    return Err(replay_error(function, "selected lexical replay does not admit nominal or callable expression storage"));
                }
                match &expression.kind {
                    ResolvedExprKind::Place(_) if expression.ty == ResolvedType::String => {
                        return Err(replay_error(function, "selected lexical replay String-place condition indexes are not reserved"));
                    }
                    ResolvedExprKind::LiteralFormat { .. }
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
                frames[depth - 1] = Some((expression, 0));
                continue;
            }
            let width = match &expression.kind {
                ResolvedExprKind::Block { statements, .. } => statements.len(),
                ResolvedExprKind::Match { arms, .. } => arms.len(),
                _ => 0,
            };
            budget.charge(function, width, "selected primitive child lookup")?;
            if let Some(child) = crate::hir::lexical_exit::child(expression, next) {
                if depth == DEPTH {
                    return Err(replay_error(
                        function,
                        "selected primitive profile exceeds ordinary HIR depth",
                    ));
                }
                let next = next.checked_add(1).ok_or_else(|| {
                    replay_error(function, "selected primitive child index overflow")
                })?;
                frames[depth - 1] = Some((expression, next));
                frames[depth] = Some((child, usize::MAX));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
    }
    Ok(())
}
