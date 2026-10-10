//! Open, authored-order, fixed-depth traversal shared by lexical proof consumers.
use crate::diagnostic::Diagnostic;
use crate::hir::{ResolvedExpr, ResolvedExprKind as K, ResolvedFunction};

pub(crate) fn child(expr: &ResolvedExpr, index: usize) -> Option<&ResolvedExpr> {
    match &expr.kind {
        K::LexicalExit { value, .. } => (index == 0).then(|| value.as_deref()).flatten(),
        K::Closure { captures, body, .. } => captures
            .get(index)
            .map(|capture| &capture.value)
            .or_else(|| (index == captures.len()).then_some(body.as_ref())),
        K::Block { statements, tail } => {
            let mut offset = 0;
            for statement in statements {
                let count = statement.child_count();
                if index < offset + count {
                    return statement.child(index - offset);
                }
                offset += count;
            }
            (index == offset).then_some(tail.as_ref())
        }
        K::Invoke { callable, args } => {
            if index == 0 {
                Some(callable)
            } else {
                args.get(index - 1)
            }
        }
        K::Call { args, .. } | K::LiteralFormat { args, .. } | K::VecFieldRead { args, .. } => {
            args.get(index)
        }
        K::NativeRustImportCall(call) => call.args.get(index),
        K::HostCommandCall(call) => call.args.get(index),
        K::ByteRange {
            source, start, end, ..
        } => [source.as_ref(), start.as_ref(), end.as_ref()]
            .get(index)
            .copied(),
        K::Unary { value, .. }
        | K::Try { operand: value, .. }
        | K::TryOption { operand: value, .. }
        | K::Project { base: value, .. }
        | K::Upcast { source: value }
        | K::Yield { request: value } => (index == 0).then_some(value),
        K::Binary { left, right, .. } => [left.as_ref(), right.as_ref()].get(index).copied(),
        K::If {
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
        K::ConstructRecord { fields, .. } | K::ConstructVariant { fields, .. } => {
            fields.get(index).map(|field| &field.value)
        }
        K::Match {
            scrutinee, arms, ..
        } => {
            if index == 0 {
                return Some(scrutinee);
            }
            let mut offset = 1;
            for arm in arms {
                if let Some(guard) = &arm.guard {
                    if index == offset {
                        return Some(guard);
                    }
                    offset += 1;
                }
                if index == offset {
                    return Some(&arm.value);
                }
                offset += 1;
            }
            None
        }
        K::UpdateRecord { base, fields, .. } => {
            if index == 0 {
                Some(base)
            } else {
                fields.get(index - 1).map(|field| &field.value)
            }
        }
        K::FunctionReference { .. }
        | K::Int(_)
        | K::Int32(_)
        | K::Char(_)
        | K::Uint8(_)
        | K::Usize(_)
        | K::ArrayU8(_)
        | K::RepeatArrayU8 { .. }
        | K::Float32(_)
        | K::Float64(_)
        | K::Bool(_)
        | K::String(_)
        | K::Place(_)
        | K::BorrowPlace { .. } => None,
    }
}

/// Finite depth-first scan: no frontier allocation, identity clones or recursion.
/// Callers charge this one source-defined function walk in their existing work census.
pub(crate) fn requires(function: &ResolvedFunction) -> Result<bool, Diagnostic> {
    let mut stack = [None; crate::cache_codec::MAX_DEPTH + 1];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        stack[0] = Some((root, 0usize));
        let mut len = 1usize;
        while len != 0 {
            let (expr, index) = stack[len - 1].expect("occupied lexical scan frame");
            if index == 0 && matches!(&expr.kind, K::LexicalExit { .. }) {
                return Ok(true);
            }
            if let Some(next) = child(expr, index) {
                if len == stack.len() {
                    return Err(Diagnostic::io(
                        "SPX-H006",
                        "lexical control scan exceeds admitted expression depth",
                    ));
                }
                stack[len - 1] = Some((expr, index + 1));
                stack[len] = Some((next, 0));
                len += 1;
            } else {
                stack[len - 1] = None;
                len -= 1;
            }
        }
    }
    Ok(false)
}

/// Admitted resolver output already obeys the HIR depth bound.
pub(crate) fn contains(root: &ResolvedExpr) -> bool {
    let mut stack = [None; crate::cache_codec::MAX_DEPTH + 1];
    stack[0] = Some((root, 0usize));
    let mut len = 1;
    while len != 0 {
        let (expr, index) = stack[len - 1].expect("occupied lexical frame");
        if index == 0 && matches!(expr.kind, K::LexicalExit { .. }) {
            return true;
        }
        if let Some(next) = child(expr, index) {
            if len == stack.len() {
                return true;
            } // fail closed: independent HIR depth replay rejects it
            stack[len - 1] = Some((expr, index + 1));
            stack[len] = Some((next, 0));
            len += 1;
        } else {
            len -= 1;
            stack[len] = None;
        }
    }
    false
}
