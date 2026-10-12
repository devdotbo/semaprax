//! Closed expression-child order for cleanup replay.
use super::*;

/// Preserve cleanup-specific child domains while giving Block/Match a
/// borrowed incremental cursor. Other shapes retain the closed replay table,
/// notably Closure captures and Invoke arguments (not their body/callable).
#[derive(Clone, Copy)]
pub(super) enum ReplayChildCursor<'a> {
    Open(crate::hir::lexical_exit::ChildCursor<'a>),
    Indexed(&'a ResolvedExpr, usize),
}
impl<'a> ReplayChildCursor<'a> {
    pub(super) fn new(expression: &'a ResolvedExpr) -> Self {
        if matches!(
            expression.kind,
            ResolvedExprKind::Block { .. }
                | ResolvedExprKind::VecConstructor { .. }
                | ResolvedExprKind::Match { .. }
        ) {
            Self::Open(crate::hir::lexical_exit::ChildCursor::new(expression))
        } else {
            Self::Indexed(expression, 0)
        }
    }
    pub(super) fn expression(&self) -> &'a ResolvedExpr {
        match self {
            Self::Open(cursor) => cursor.expression(),
            Self::Indexed(expression, _) => expression,
        }
    }
    pub(super) fn next(&mut self) -> Option<&'a ResolvedExpr> {
        match self {
            Self::Open(cursor) => cursor.next(),
            Self::Indexed(expression, next) => {
                let child = replay_expression_child(expression, *next);
                if child.is_some() {
                    *next += 1;
                }
                child
            }
        }
    }
}

pub(super) fn replay_expression_child(
    expression: &ResolvedExpr,
    index: usize,
) -> Option<&ResolvedExpr> {
    if let Some((_, args)) = crate::hir::closure::once::call(expression) {
        return args.get(index);
    }
    match &expression.kind {
        ResolvedExprKind::LexicalExit { value, .. } => {
            (index == 0).then(|| value.as_deref()).flatten()
        }
        ResolvedExprKind::Closure { captures, .. } => {
            captures.get(index).map(|capture| &capture.value)
        }
        ResolvedExprKind::FunctionReference { .. } => None,
        ResolvedExprKind::Invoke { args, .. } => args.get(index),
        ResolvedExprKind::Call { args, .. }
        | ResolvedExprKind::LiteralFormat { args, .. }
        | ResolvedExprKind::VecFieldRead { args, .. } => args.get(index),
        ResolvedExprKind::NativeRustImportCall(call) => call.args.get(index),
        ResolvedExprKind::HostCommandCall(call) => call.args.get(index),
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => [source.as_ref(), start.as_ref(), end.as_ref()]
            .get(index)
            .copied(),
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Yield { request: value } => (index == 0).then_some(value),
        ResolvedExprKind::Binary { left, right, .. } => {
            [left.as_ref(), right.as_ref()].get(index).copied()
        }
        ResolvedExprKind::Block { statements, tail }
        | ResolvedExprKind::VecConstructor {
            statements, tail, ..
        } => {
            let mut offset = 0;
            for statement in statements {
                let count = statement.child_count();
                if index < offset + count {
                    return statement.child(index - offset);
                }
                offset += count;
            }
            (index == offset).then_some(tail)
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => [
            condition.as_ref(),
            then_branch.as_ref(),
            else_branch.as_ref(),
        ]
        .get(index)
        .copied(),
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            fields.get(index).map(|field| &field.value)
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            if index == 0 {
                Some(scrutinee.as_ref())
            } else {
                // Refutable Match v1: each arm contributes its optional
                // guard first, then its value.
                let mut cursor = index - 1;
                for arm in arms {
                    if let Some(guard) = &arm.guard {
                        if cursor == 0 {
                            return Some(guard.as_ref());
                        }
                        cursor -= 1;
                    }
                    if cursor == 0 {
                        return Some(&arm.value);
                    }
                    cursor -= 1;
                }
                None
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => (index == 0)
            .then_some(base.as_ref())
            .or_else(|| fields.get(index - 1).map(|field| &field.value)),
        ResolvedExprKind::Int(_)
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
        | ResolvedExprKind::BorrowPlace { .. }
        // Row Loan v1 is a cleanup-inert borrowed leaf over two exact places.
        | ResolvedExprKind::RowLoan { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{replay_expression_child, ReplayChildCursor};
    use crate::hir::{ResolvedExpr, ResolvedExprKind};

    #[test]
    fn cursor_preserves_closed_child_order_and_invoke_argument_domain() {
        let checked = crate::check(
            include_str!("../../../tests/owned_data/byte_helper_renewal.spx"),
            "replay-child-cursor.spx",
        )
        .unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let mut pending = Vec::new();
        for function in &program.functions {
            pending.extend(function.requires.iter());
            pending.push(&function.body);
            pending.extend(function.ensures.iter());
        }
        while let Some(expression) = pending.pop() {
            let expected: Vec<_> = (0..)
                .map_while(|i| replay_expression_child(expression, i))
                .collect();
            let mut cursor = ReplayChildCursor::new(expression);
            assert!(std::ptr::eq(cursor.expression(), expression));
            for child in &expected {
                assert!(std::ptr::eq(cursor.next().unwrap(), *child));
            }
            assert!(cursor.next().is_none());
            assert!(cursor.next().is_none());
            pending.extend(expected);
        }
        let main = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let mut invocation: ResolvedExpr = main.body.clone();
        invocation.kind = ResolvedExprKind::Invoke {
            callable: Box::new(main.body.clone()),
            args: vec![main.body.clone()],
        };
        let ResolvedExprKind::Invoke { callable, args } = &invocation.kind else {
            unreachable!()
        };
        let mut cursor = ReplayChildCursor::new(&invocation);
        assert!(std::ptr::eq(cursor.next().unwrap(), &args[0]));
        assert!(cursor.next().is_none());
        assert!(!std::ptr::eq(&args[0], callable.as_ref()));
    }
}
