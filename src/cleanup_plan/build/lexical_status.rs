//! Constructor-owned status declaration in typed authored occurrence order.
//! CFG visits and finite resume copies consume these declarations, never reorder them.
use super::*;
mod admission;
use crate::hir::lexical_exit::ChildCursor;
use std::cell::Cell;
use std::mem::size_of;
const DEPTH: usize = 514;
const WORK: usize = 32_000_000;
#[derive(Clone, Copy)]
struct Frame<'a> {
    expression: &'a ResolvedExpr,
    cursor: ChildCursor<'a>,
    special: usize,
}
#[derive(Clone, Copy)]
enum Producer<'a> {
    Call(&'a str),
    Arithmetic(CheckedOperation, &'static [StatusCase]),
    Contract(ContractPhase, u32),
}
fn debit(work: &Cell<usize>, count: usize) -> Result<(), Diagnostic> {
    work.set(
        work.get()
            .checked_sub(count)
            .ok_or_else(|| plan_error("lexical status census work exhausted"))?,
    );
    Ok(())
}
fn reserve(bytes: usize) -> Result<(), Diagnostic> {
    if crate::bounded_output::reserve_active_required(bytes) {
        Ok(())
    } else {
        Err(plan_error(
            "lexical status census exceeds physical construction budget",
        ))
    }
}
fn next<'a>(frame: &mut Frame<'a>) -> Option<&'a ResolvedExpr> {
    // Callable bodies have their own execution domain. Only evaluated capture/
    // argument expressions participate in this function's status inventory.
    let result = if let Some((_, args)) = crate::hir::closure::once::call(frame.expression) {
        args.get(frame.special)
    } else {
        match &frame.expression.kind {
            ResolvedExprKind::Closure { captures, .. } => {
                captures.get(frame.special).map(|c| &c.value)
            }
            ResolvedExprKind::Invoke { args, .. } => args.get(frame.special),
            ResolvedExprKind::FunctionReference { .. } => None,
            _ => return frame.cursor.next(),
        }
    };
    if result.is_some() {
        frame.special += 1;
    }
    result
}
fn expression<'a>(
    program: &'a ResolvedProgram,
    value: &'a ResolvedExpr,
    work: &Cell<usize>,
    emit: &mut impl FnMut(&ExpressionId, StatusLane, Producer<'_>) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    if admission::required(program, value, work)? {
        let call = super::super::native_rust::parts(value).expect("typed admission call");
        emit(
            &value.id,
            StatusLane::OwnerAdmission,
            Producer::Call(call.callee.as_str()),
        )?;
    }
    let operation = match &value.kind {
        ResolvedExprKind::ByteRange { operation, .. } => {
            if operation.as_str() != crate::byte_ops::RANGE_ID {
                return Err(plan_error("unknown lexical byte-range status operation"));
            }
            Some(Producer::Call(operation.as_str()))
        }
        ResolvedExprKind::Invoke { .. } | ResolvedExprKind::Closure { .. }
            if crate::hir::closure::once::call(value).is_some() =>
        {
            Some(Producer::Call(
                crate::hir::closure::once::call(value)
                    .expect("affine call")
                    .0
                    .as_str(),
            ))
        }
        ResolvedExprKind::Invoke { .. } => Some(Producer::Call("core.function.invoke")),
        ResolvedExprKind::VecFieldRead { .. } => Some(Producer::Call(crate::vec_field::ID)),
        ResolvedExprKind::LiteralFormat { .. } => Some(Producer::Call(crate::literal_format::ID)),
        ResolvedExprKind::Call {
            callee, instance, ..
        } => {
            let id = callee.as_str();
            if instance.is_none()
                && (crate::byte_ops::by_id(id).is_some_and(|op| !op.is_fallible())
                    || crate::host_io_ops::by_id(id).is_some()
                    || id == crate::stdin_stream_ops::EOF_ID
                    || super::super::deferred_commit::is_infallible_vec_operation(
                        crate::vec_ops::by_id(id),
                    )
                    || super::super::deferred_commit::is_infallible_box_operation(callee))
            {
                None
            } else {
                if !(instance.is_none()
                    && (crate::map_ops::by_id(id).is_some()
                        || crate::string_ops::by_id(id).is_some()
                        || crate::str_ops::by_id(id).is_some()
                        || crate::vec_ops::by_id(id).is_some()
                        || crate::iterator_ops::by_id(id).is_some()
                        || crate::list_ops::by_id(id).is_some()
                        || crate::box_ops::by_id(id).is_some()
                        || crate::byte_ops::by_id(id).is_some()))
                    && {
                        debit(
                            work,
                            if instance.is_some() {
                                program.function_instances.len()
                            } else {
                                program.functions.len()
                            },
                        )?;
                        program
                            .resolve_call_target(callee, instance.as_ref())
                            .is_none()
                    }
                {
                    return Err(plan_error("lexical status call has no typed target"));
                }
                Some(Producer::Call(id))
            }
        }
        ResolvedExprKind::HostCommandCall(call)
            if crate::command_io_ops::failure(call.operation)
                == crate::command_io_ops::CommandIoFailure::Status =>
        {
            Some(Producer::Call(crate::command_io_ops::id(call.operation)))
        }
        ResolvedExprKind::NativeRustImportCall(call) if super::super::native_rust::owns(value) => {
            Some(Producer::Call(call.import.as_str()))
        }
        ResolvedExprKind::Unary {
            op: UnaryOp::Neg, ..
        } => Some(Producer::Arithmetic(
            CheckedOperation::Neg,
            &[StatusCase::NegationOverflow],
        )),
        ResolvedExprKind::Binary { op, .. } => match op {
            BinaryOp::Add => Some(Producer::Arithmetic(
                CheckedOperation::Add,
                &[StatusCase::AddOverflow],
            )),
            BinaryOp::Sub => Some(Producer::Arithmetic(
                CheckedOperation::Sub,
                &[StatusCase::SubOverflow],
            )),
            BinaryOp::Mul => Some(Producer::Arithmetic(
                CheckedOperation::Mul,
                &[StatusCase::MulOverflow],
            )),
            BinaryOp::Div => Some(Producer::Arithmetic(
                CheckedOperation::Div,
                &[StatusCase::DivisionByZero, StatusCase::DivisionOverflow],
            )),
            BinaryOp::Rem => Some(Producer::Arithmetic(
                CheckedOperation::Rem,
                &[StatusCase::RemainderByZero, StatusCase::RemainderOverflow],
            )),
            BinaryOp::Eq
            | BinaryOp::Ne
            | BinaryOp::Lt
            | BinaryOp::Le
            | BinaryOp::Gt
            | BinaryOp::Ge
            | BinaryOp::And
            | BinaryOp::Or => None,
        },
        ResolvedExprKind::Closure { .. }
        | ResolvedExprKind::FunctionReference { .. }
        | ResolvedExprKind::HostCommandCall(_)
        | ResolvedExprKind::NativeRustImportCall(_)
        | ResolvedExprKind::Unary {
            op: UnaryOp::Not, ..
        }
        | ResolvedExprKind::Block { .. }
        | ResolvedExprKind::LexicalExit { .. }
        | ResolvedExprKind::If { .. }
        | ResolvedExprKind::ConstructRecord { .. }
        | ResolvedExprKind::ConstructVariant { .. }
        | ResolvedExprKind::Try { .. }
        | ResolvedExprKind::TryOption { .. }
        | ResolvedExprKind::Match { .. }
        | ResolvedExprKind::UpdateRecord { .. }
        | ResolvedExprKind::Project { .. }
        | ResolvedExprKind::Upcast { .. }
        | ResolvedExprKind::Yield { .. }
        | ResolvedExprKind::Int(_)
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
        | ResolvedExprKind::BorrowPlace { .. } => None,
    };
    if let Some(producer) = operation {
        emit(&value.id, StatusLane::OperationFailure, producer)?;
    }
    Ok(())
}
fn walk<'a>(
    program: &'a ResolvedProgram,
    root: &'a ResolvedExpr,
    frames: &mut [Option<Frame<'a>>; DEPTH],
    work: &Cell<usize>,
    emit: &mut impl FnMut(&ExpressionId, StatusLane, Producer<'_>) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    if let ResolvedExprKind::Block { statements, .. } = &root.kind {
        debit(work, statements.len())?;
    }
    frames[0] = Some(Frame {
        expression: root,
        cursor: ChildCursor::new(root),
        special: 0,
    });
    let mut depth = 1;
    while depth != 0 {
        debit(work, 1)?;
        let frame = frames[depth - 1].as_mut().expect("status frame");
        if let Some(child) = next(frame) {
            if depth == DEPTH {
                return Err(plan_error("lexical status expression depth exceeds 512"));
            }
            if let ResolvedExprKind::Block { statements, .. } = &child.kind {
                debit(work, statements.len())?;
            }
            frames[depth] = Some(Frame {
                expression: child,
                cursor: ChildCursor::new(child),
                special: 0,
            });
            depth += 1;
        } else {
            let frame = frames[depth - 1].take().expect("finished status frame");
            depth -= 1;
            expression(program, frame.expression, work, emit)?;
        }
    }
    Ok(())
}
fn visit<'a>(
    program: &'a ResolvedProgram,
    function: &'a ResolvedFunction,
    frames: &mut [Option<Frame<'a>>; DEPTH],
    work: &Cell<usize>,
    emit: &mut impl FnMut(&ExpressionId, StatusLane, Producer<'_>) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    for (phase, contracts) in [
        (ContractPhase::Requires, function.requires.as_slice()),
        (ContractPhase::Ensures, function.ensures.as_slice()),
    ] {
        if phase == ContractPhase::Ensures {
            walk(program, &function.body, frames, work, emit)?;
        }
        for (ordinal, contract) in contracts.iter().enumerate() {
            walk(program, contract, frames, work, emit)?;
            emit(
                &contract.id,
                StatusLane::ContractFalse,
                Producer::Contract(
                    phase,
                    u32::try_from(ordinal).map_err(|_| plan_error("too many lexical contracts"))?,
                ),
            )?;
        }
    }
    Ok(())
}
pub(super) fn declare(
    program: &ResolvedProgram,
    function: &ResolvedFunction,
) -> Result<(Vec<StatusSource>, Vec<u8>, usize), Diagnostic> {
    reserve(size_of::<[Option<Frame<'_>>; DEPTH]>())?;
    let mut frames = [None; DEPTH];
    let work = Cell::new(WORK);
    let mut count = 0usize;
    let mut payload = 0usize;
    visit(
        program,
        function,
        &mut frames,
        &work,
        &mut |_, _, producer| {
            count = count
                .checked_add(1)
                .ok_or_else(|| plan_error("lexical status count overflow"))?;
            let bytes = match producer {
                Producer::Call(id) => id.len(),
                Producer::Arithmetic(_, cases) => cases.len() * size_of::<StatusCase>(),
                Producer::Contract(..) => 0,
            };
            payload = payload
                .checked_add(bytes)
                .ok_or_else(|| plan_error("lexical status payload overflow"))?;
            Ok(())
        },
    )?;
    let bytes = count
        .checked_mul(size_of::<StatusSource>() + 1)
        .and_then(|b| b.checked_add(payload))
        .ok_or_else(|| plan_error("lexical status backing overflow"))?;
    reserve(bytes)?;
    let mut sources = Vec::new();
    sources
        .try_reserve_exact(count)
        .map_err(|_| plan_error("lexical status backing unavailable"))?;
    if sources.capacity() != count {
        return Err(plan_error("lexical status backing capacity differs"));
    }
    visit(
        program,
        function,
        &mut frames,
        &work,
        &mut |expression, lane, producer| {
            debit(&work, sources.len())?;
            if sources
                .iter()
                .any(|s: &StatusSource| s.id.expression == *expression && s.id.lane == lane)
            {
                return Err(plan_error("duplicate authored lexical status identity"));
            }
            let producer = match producer {
                Producer::Call(id) => StatusProducer::PropagatedCall {
                    callee: DeclarationId::new(id),
                },
                Producer::Arithmetic(operation, cases) => StatusProducer::CheckedArithmetic {
                    operation,
                    normalized_cases: cases.to_vec(),
                },
                Producer::Contract(phase, ordinal) => {
                    StatusProducer::ContractFalse { phase, ordinal }
                }
            };
            sources.push(StatusSource {
                id: StatusSourceId {
                    expression: expression.clone(),
                    lane,
                },
                producer,
            });
            Ok(())
        },
    )?;
    let seen = vec![0; count];
    Ok((sources, seen, work.get()))
}

#[cfg(test)]
mod tests;
