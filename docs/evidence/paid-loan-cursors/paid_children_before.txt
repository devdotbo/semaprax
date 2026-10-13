//! Identical discovery child order with caller-paid replacement backing.
use super::*;

pub(super) fn push_children<'a>(
    expression: &'a ResolvedExpr,
    pending: &mut Vec<&'a ResolvedExpr>,
    payer: Option<&crate::bounded_output::cfg_reservation::CfgReservation>,
) -> Result<(), Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::LexicalExit { value, .. } => {
            storage::extend(pending, value.as_deref(), payer)?;
        }
        ResolvedExprKind::Closure { captures, .. } => storage::extend(
            pending,
            captures.iter().rev().map(|capture| &capture.value),
            payer,
        )?,
        ResolvedExprKind::FunctionReference { .. } => {}
        ResolvedExprKind::Invoke { callable, args } => {
            storage::extend(pending, args.iter().rev(), payer)?;
            storage::push(pending, callable, payer)?;
        }
        ResolvedExprKind::Block { statements, tail }
        | ResolvedExprKind::VecConstructor {
            statements, tail, ..
        } => {
            storage::push(pending, tail, payer)?;
            for statement in statements.iter().rev() {
                for index in (0..statement.child_count()).rev() {
                    if let Some(child) = statement.child(index) {
                        storage::push(pending, child, payer)?;
                    }
                }
            }
        }
        ResolvedExprKind::Call { args, .. }
        | ResolvedExprKind::LiteralFormat { args, .. }
        | ResolvedExprKind::VecFieldRead { args, .. }
        | ResolvedExprKind::RowLoan { args, .. } => {
            storage::extend(pending, args.iter().rev(), payer)?
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            storage::extend(pending, call.args.iter().rev(), payer)?
        }
        ResolvedExprKind::HostCommandCall(call) => {
            storage::extend(pending, call.args.iter().rev(), payer)?
        }
        ResolvedExprKind::ByteRange {
            source, start, end, ..
        } => {
            storage::push(pending, end, payer)?;
            storage::push(pending, start, payer)?;
            storage::push(pending, source, payer)?;
        }
        ResolvedExprKind::Unary { value, .. }
        | ResolvedExprKind::Try { operand: value, .. }
        | ResolvedExprKind::TryOption { operand: value, .. }
        | ResolvedExprKind::Project { base: value, .. }
        | ResolvedExprKind::Upcast { source: value }
        | ResolvedExprKind::Yield { request: value } => storage::push(pending, value, payer)?,
        ResolvedExprKind::Binary { left, right, .. } => {
            storage::push(pending, right, payer)?;
            storage::push(pending, left, payer)?;
        }
        ResolvedExprKind::If {
            condition,
            then_branch,
            else_branch,
        } => {
            storage::push(pending, else_branch, payer)?;
            storage::push(pending, then_branch, payer)?;
            storage::push(pending, condition, payer)?;
        }
        ResolvedExprKind::ConstructRecord { fields, .. }
        | ResolvedExprKind::ConstructVariant { fields, .. } => {
            storage::extend(
                pending,
                fields.iter().rev().map(|field| &field.value),
                payer,
            )?;
        }
        ResolvedExprKind::Match {
            scrutinee, arms, ..
        } => {
            for arm in arms.iter().rev() {
                storage::push(pending, &arm.value, payer)?;
                if let Some(guard) = &arm.guard {
                    storage::push(pending, guard, payer)?;
                }
            }
            storage::push(pending, scrutinee, payer)?;
        }
        ResolvedExprKind::UpdateRecord { base, fields, .. } => {
            storage::extend(
                pending,
                fields.iter().rev().map(|field| &field.value),
                payer,
            )?;
            storage::push(pending, base, payer)?;
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
