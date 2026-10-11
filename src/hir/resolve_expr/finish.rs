//! Finish resolver result stacks and already-resolved class-prefix projections.
use super::*;
impl Resolver<'_> {
    pub(super) fn finish_upcast(
        &self,
        function: &FunctionExecutionId,
        path: String,
        holder: DeclarationId,
        span: crate::ast::Span,
        source: ResolvedExpr,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let declared = ResolvedType::Nominal {
            declaration: holder,
            arguments: Vec::new(),
        };
        Ok(ResolvedExpr {
            id: ExpressionId::new(function, &path),
            ty: declared.clone(),
            ownership: self.expression_ownership(&declared, OwnershipMode::Own, span)?,
            kind: ResolvedExprKind::Upcast {
                source: Box::new(source),
            },
            span,
        })
    }
}

impl Resolver<'_> {
    pub(super) fn finish_expression_results(
        &self,
        mut results: Vec<ResolvedExpr>,
        span: crate::ast::Span,
    ) -> Result<ResolvedExpr, Diagnostic> {
        if results.len() != 1 {
            return Err(self.error(
                "SPX-H006",
                "iterative expression resolver finished with an invalid result stack",
                span,
            ));
        }
        results.pop().ok_or_else(|| {
            self.error(
                "SPX-H006",
                "iterative expression resolver lost its root result",
                span,
            )
        })
    }
}

pub(super) fn finish_if(
    function: &FunctionExecutionId,
    span: crate::ast::Span,
    path: String,
    children: Vec<ResolvedExpr>,
) -> ResolvedExpr {
    let mut children = children.into_iter();
    let condition = children.next().expect("if condition retained");
    let then_branch = children.next().expect("if then branch retained");
    let else_branch = children.next().expect("if else branch retained");
    let normal = if then_branch.ty == ResolvedType::Never {
        &else_branch
    } else {
        &then_branch
    };
    ResolvedExpr {
        id: ExpressionId::new(function, &path),
        ty: normal.ty.clone(),
        ownership: normal.ownership,
        kind: ResolvedExprKind::If {
            condition: Box::new(condition),
            then_branch: Box::new(then_branch),
            else_branch: Box::new(else_branch),
        },
        span,
    }
}

pub(super) fn finish_block(
    function: &FunctionExecutionId,
    span: crate::ast::Span,
    path: String,
    statements: Vec<ResolvedStatement>,
    tail: ResolvedExpr,
) -> ResolvedExpr {
    ResolvedExpr {
        id: ExpressionId::new(function, &path),
        ty: tail.ty.clone(),
        ownership: tail.ownership,
        kind: ResolvedExprKind::Block {
            statements,
            tail: Box::new(tail),
        },
        span,
    }
}

pub(super) fn finish_unary(
    function: &FunctionExecutionId,
    span: crate::ast::Span,
    path: String,
    op: UnaryOp,
    value: ResolvedExpr,
) -> ResolvedExpr {
    let ty = match (&op, &value.ty) {
        (UnaryOp::Neg, ResolvedType::F32) => ResolvedType::F32,
        (UnaryOp::Neg, ResolvedType::F64) => ResolvedType::F64,
        (UnaryOp::Neg, ResolvedType::I32) => ResolvedType::I32,
        (UnaryOp::Neg, _) => ResolvedType::I64,
        (UnaryOp::Not, _) => ResolvedType::Bool,
    };
    ResolvedExpr {
        id: ExpressionId::new(function, &path),
        ty,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::Unary {
            op,
            value: Box::new(value),
        },
        span,
    }
}

impl Resolver<'_> {
    pub(super) fn finish_lexical_match(
        &self,
        function: &FunctionExecutionId,
        span: crate::ast::Span,
        path: String,
        mode: ResolvedMatchMode,
        scrutinee: ResolvedExpr,
        arms: Vec<ResolvedMatchArm>,
    ) -> Result<ResolvedExpr, Diagnostic> {
        let first = super::super::lexical_exit::normal_arm(&arms)
            .ok_or_else(|| self.error("SPX-H006", "resolved match has no arms", span))?;
        Ok(ResolvedExpr {
            id: ExpressionId::new(function, &path),
            ty: first.value.ty.clone(),
            ownership: first.value.ownership,
            kind: ResolvedExprKind::Match {
                mode,
                scrutinee: Box::new(scrutinee),
                arms,
            },
            span,
        })
    }
}

pub(super) fn start_upcast<'a>(frame: Frame<'a>, frames: &mut Vec<Frame<'a>>) {
    let Frame::StartUpcast {
        source,
        bindings,
        slot_path,
        holder,
        span,
        resume,
    } = frame
    else {
        unreachable!("upcast continuation")
    };
    // Re-resolve the consumed expression at the canonical `.source` identity below its occupied slot, then wrap and resume the interrupted
    // continuation.
    frames.push(*resume);
    frames.push(Frame::FinishUpcast {
        slot_path: slot_path.clone(),
        holder,
        span,
    });
    frames.push(Frame::Enter {
        expr: source,
        bindings,
        path: format!("{slot_path}.source"),
    });
}
