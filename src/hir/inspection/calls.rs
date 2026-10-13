//! Borrowed call preorder; a refusal stops before subsequent call activity.
use super::*;

pub(crate) fn try_visit_resolved_calls<'e, E>(
    expression: &'e ResolvedExpr,
    visit: &mut impl FnMut(
        &'e DeclarationId,
        Option<&'e FunctionInstanceId>,
        &'e [ResolvedType],
    ) -> Result<(), E>,
) -> Result<(), E> {
    match &expression.kind {
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            try_visit_resolved_calls(source, visit)?;
            try_visit_resolved_calls(start, visit)?;
            try_visit_resolved_calls(end, visit)?;
        }
        ResolvedExprKind::Closure { body, captures, .. } => {
            try_visit_resolved_calls(body, visit)?;
            for capture in captures {
                try_visit_resolved_calls(&capture.value, visit)?;
            }
        }
        ResolvedExprKind::FunctionReference { target } => visit(target, None, &[])?,
        ResolvedExprKind::Invoke { callable, args } => {
            try_visit_resolved_calls(callable, visit)?;
            for arg in args {
                try_visit_resolved_calls(arg, visit)?;
            }
        }
        ResolvedExprKind::Call {
            callee,
            instance,
            type_arguments,
            args,
        } => {
            visit(callee, instance.as_ref(), type_arguments)?;
            for arg in args {
                try_visit_resolved_calls(arg, visit)?;
            }
        }
        ResolvedExprKind::LiteralFormat { args, .. }
        | ResolvedExprKind::VecFieldRead { args, .. }
        | ResolvedExprKind::RowLoan { args, .. } => {
            for arg in args {
                try_visit_resolved_calls(arg, visit)?;
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            for arg in &call.args {
                try_visit_resolved_calls(arg, visit)?;
            }
        }
        ResolvedExprKind::HostCommandCall(call) => {
            for arg in &call.args {
                try_visit_resolved_calls(arg, visit)?;
            }
        }
        ResolvedExprKind::LexicalExit { value, .. } => {
            if let Some(value) = value {
                try_visit_resolved_calls(value, visit)?;
            }
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Yield { request: value } => try_visit_resolved_calls(value, visit)?,
        ResolvedExprKind::Binary { left, right, .. } => {
            try_visit_resolved_calls(left, visit)?;
            try_visit_resolved_calls(right, visit)?;
        }
        ResolvedExprKind::Block { statements, tail }
        | ResolvedExprKind::VecConstructor {
            statements, tail, ..
        } => {
            for statement in statements {
                for index in 0..statement.child_count() {
                    if let Some(child) = statement.child(index) {
                        try_visit_resolved_calls(child, visit)?;
                    }
                }
            }
            try_visit_resolved_calls(tail, visit)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            try_visit_resolved_calls(condition, visit)?;
            try_visit_resolved_calls(then_branch, visit)?;
            try_visit_resolved_calls(else_branch, visit)?;
        }
        ResolvedExprKind::ConstructRecord { fields, .. } => {
            for field in fields {
                try_visit_resolved_calls(&field.value, visit)?;
            }
        }
        ResolvedExprKind::ConstructVariant { fields, .. } => {
            for field in fields {
                try_visit_resolved_calls(&field.value, visit)?;
            }
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            try_visit_resolved_calls(scrutinee, visit)?;
            for arm in arms {
                // A guard is authored before the arm value and, under left to
                // right evaluation, runs before it. Skipping it would hide a
                // callee reached only from a guard.
                if let Some(guard) = &arm.guard {
                    try_visit_resolved_calls(guard, visit)?;
                }
                try_visit_resolved_calls(&arm.value, visit)?;
            }
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            try_visit_resolved_calls(base, visit)?;
            for field in fields {
                try_visit_resolved_calls(&field.value, visit)?;
            }
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
    Ok(())
}

#[cfg(test)]
mod tests;
