//! Target capture belongs to the resolver contour, never an authored name.
use super::*;
use crate::ast::{LexicalExitKind, Span};
use crate::hir::{ControlTarget, LoopLatchKind};
pub(super) fn schedule<'a>(
    function: &FunctionExecutionId,
    expr: &'a Expr,
    kind: LexicalExitKind,
    value: Option<&'a Expr>,
    bindings: &Rc<BTreeMap<String, Binding>>,
    path: &str,
    nearest: Option<&ControlTarget>,
    frames: &mut Vec<Frame<'a>>,
) -> Result<(), Diagnostic> {
    let target = match kind {
        LexicalExitKind::Return => ControlTarget::Function(function.clone()),
        LexicalExitKind::Break | LexicalExitKind::Continue => nearest
            .cloned()
            .ok_or_else(|| Diagnostic::io("SPX-T313", "lexical loop exit has no enclosing loop"))?,
    };
    if (kind == LexicalExitKind::Return) != value.is_some() {
        return Err(Diagnostic::io(
            "SPX-T312",
            "lexical exit operand differs from its kind",
        ));
    }
    frames.push(Frame::FinishLexicalExit {
        span: expr.span,
        path: path.into(),
        kind,
        target,
        has_value: value.is_some(),
    });
    if let Some(value) = value {
        frames.push(Frame::Enter {
            expr: value,
            bindings: bindings.clone(),
            path: format!("{path}.value"),
        });
    }
    Ok(())
}
pub(super) fn finish(
    function: &FunctionExecutionId,
    span: Span,
    path: &str,
    kind: LexicalExitKind,
    target: ControlTarget,
    value: Option<Box<ResolvedExpr>>,
) -> ResolvedExpr {
    ResolvedExpr {
        id: ExpressionId::new(function, path),
        ty: ResolvedType::Never,
        ownership: OwnershipMode::Value,
        kind: ResolvedExprKind::LexicalExit {
            kind,
            target,
            value,
        },
        span,
    }
}
pub(in crate::hir) fn loop_statement(
    condition: Box<ResolvedExpr>,
    body: Box<ResolvedExpr>,
    latch: LoopLatchKind,
    span: Span,
) -> ResolvedStatement {
    if crate::hir::lexical_exit::contains(&body) {
        ResolvedStatement::LexicalWhile {
            condition,
            body,
            latch,
            span,
        }
    } else {
        ResolvedStatement::While {
            condition,
            body,
            span,
        }
    }
}

pub(super) fn enter_loop(
    targets: &mut [Option<ControlTarget>],
    depth: &mut usize,
    target: ControlTarget,
) -> Result<(), Diagnostic> {
    let slot = targets
        .get_mut(*depth)
        .ok_or_else(|| Diagnostic::io("SPX-H006", "lexical loop nesting exceeds admitted depth"))?;
    *slot = Some(target);
    *depth += 1;
    Ok(())
}
