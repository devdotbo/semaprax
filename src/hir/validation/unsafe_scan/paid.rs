//! Complete unsafe scan, retaining its capture-only closure rule and early return.
use super::*;

pub(in crate::hir::validation) fn contains_unsafe_boundary_paid(
    expression: &ResolvedExpr,
) -> Result<bool, Diagnostic> {
    let mut pending = crate::hir::ordinary_scan::Stack::new()?;
    pending.push(expression)?;
    while let Some(expression) = pending.pop() {
        match &expression.kind {
            ResolvedExprKind::Closure { captures, .. } => {
                for child in captures.iter().map(|capture| &capture.value) {
                    pending.push(child)?;
                }
            }
            ResolvedExprKind::Block { statements, tail }
            | ResolvedExprKind::VecConstructor {
                statements, tail, ..
            } => {
                pending.push(tail)?;
                for statement in statements.iter().rev() {
                    if matches!(statement, ResolvedStatement::Unsafe { .. }) {
                        return Ok(true);
                    }
                    for index in (0..statement.child_count()).rev() {
                        if let Some(child) = statement.child(index) {
                            pending.push(child)?;
                        }
                    }
                }
            }
            ResolvedExprKind::FunctionReference { .. } => {}
            ResolvedExprKind::Invoke { callable, args } => {
                {
                    for child in args.iter().rev() {
                        pending.push(child)?;
                    }
                };
                pending.push(callable)?;
            }
            ResolvedExprKind::Call { args, .. }
            | ResolvedExprKind::LiteralFormat { args, .. }
            | ResolvedExprKind::VecFieldRead { args, .. }
            | ResolvedExprKind::RowLoan { args, .. } => {
                for child in args.iter().rev() {
                    pending.push(child)?;
                }
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                {
                    for child in call.args.iter().rev() {
                        pending.push(child)?;
                    }
                };
            }
            ResolvedExprKind::HostCommandCall(call) => {
                for child in call.args.iter().rev() {
                    pending.push(child)?;
                }
            }
            ResolvedExprKind::ByteRange {
                source, start, end, ..
            } => {
                pending.push(end)?;
                pending.push(start)?;
                pending.push(source)?;
            }
            ResolvedExprKind::LexicalExit { value, .. } => {
                if let Some(value) = value {
                    pending.push(value)?;
                }
            }
            ResolvedExprKind::Unary { value, .. }
            | ResolvedExprKind::Try { operand: value, .. }
            | ResolvedExprKind::TryOption { operand: value, .. }
            | ResolvedExprKind::Project { base: value, .. }
            | ResolvedExprKind::Upcast { source: value }
            | ResolvedExprKind::Yield { request: value } => pending.push(value)?,
            ResolvedExprKind::Binary { left, right, .. } => {
                pending.push(right)?;
                pending.push(left)?;
            }
            ResolvedExprKind::If {
                condition,
                then_branch,
                else_branch,
            } => {
                pending.push(else_branch)?;
                pending.push(then_branch)?;
                pending.push(condition)?;
            }
            ResolvedExprKind::ConstructRecord { fields, .. }
            | ResolvedExprKind::ConstructVariant { fields, .. } => {
                {
                    for child in fields.iter().rev().map(|field| &field.value) {
                        pending.push(child)?;
                    }
                };
            }
            ResolvedExprKind::Match {
                scrutinee, arms, ..
            } => {
                {
                    for child in arms.iter().rev().map(|arm| &arm.value) {
                        pending.push(child)?;
                    }
                };
                pending.push(scrutinee)?;
            }
            ResolvedExprKind::UpdateRecord { base, fields, .. } => {
                {
                    for child in fields.iter().rev().map(|field| &field.value) {
                        pending.push(child)?;
                    }
                };
                pending.push(base)?;
            }
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
            | ResolvedExprKind::BorrowPlace { .. } => {}
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests;
