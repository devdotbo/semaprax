//! Saved ordinary owned reconstruction, before borrowed comparison.
use super::*;
pub(super) fn validate_required_status_sources(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) -> Result<(), Diagnostic> {
    let mut expected = Vec::new();
    for (ordinal, contract) in function.requires.iter().enumerate() {
        collect_expression_statuses(program, function, contract, &mut expected)?;
        expected.push(StatusSource {
            id: StatusSourceId {
                expression: contract.id.clone(),
                lane: StatusLane::ContractFalse,
            },
            producer: StatusProducer::ContractFalse {
                phase: super::super::super::ContractPhase::Requires,
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| replay_error(function, "too many preconditions"))?,
            },
        });
    }
    collect_expression_statuses(program, function, &function.body, &mut expected)?;
    for (ordinal, contract) in function.ensures.iter().enumerate() {
        collect_expression_statuses(program, function, contract, &mut expected)?;
        expected.push(StatusSource {
            id: StatusSourceId {
                expression: contract.id.clone(),
                lane: StatusLane::ContractFalse,
            },
            producer: StatusProducer::ContractFalse {
                phase: super::super::super::ContractPhase::Ensures,
                ordinal: u32::try_from(ordinal)
                    .map_err(|_| replay_error(function, "too many postconditions"))?,
            },
        });
    }
    if function.cleanup_plan.status_sources != expected {
        return Err(replay_error(
            function,
            "cleanup status sources do not exactly cover typed HIR failure producers",
        ));
    }
    Ok(())
}

fn collect_expression_statuses(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    statuses: &mut Vec<StatusSource>,
) -> Result<(), Diagnostic> {
    let mut stack = [None; 514];
    stack[0] = Some((expression, 0usize));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (expression, next) = stack[len].take().expect("status frame retained");
        if let Some(child) = replay_expression_child(expression, next) {
            if len + 2 > stack.len() {
                return Err(replay_error(
                    function,
                    "cleanup status expression depth exceeds 512",
                ));
            }
            stack[len] = Some((expression, next + 1));
            stack[len + 1] = Some((child, 0));
            len += 2;
            continue;
        }
        if super::super::super::owner_admission::required(program, expression) {
            let call =
                super::super::super::native_rust::parts(expression).expect("admitted owner call");
            statuses.push(StatusSource {
                id: StatusSourceId {
                    expression: expression.id.clone(),
                    lane: StatusLane::OwnerAdmission,
                },
                producer: StatusProducer::PropagatedCall {
                    callee: call.callee.clone(),
                },
            });
        }
        match &expression.kind {
            ResolvedExprKind::Closure { .. } if !expression.ty.is_once_function() => {}
            ResolvedExprKind::FunctionReference { .. } => {}
            ResolvedExprKind::ByteRange { operation, .. } => {
                if operation.as_str() != crate::byte_ops::RANGE_ID {
                    return Err(replay_error(
                        function,
                        "byte range status source carries an unknown operation identity",
                    ));
                }
                statuses.push(StatusSource {
                    id: StatusSourceId {
                        expression: expression.id.clone(),
                        lane: StatusLane::OperationFailure,
                    },
                    producer: StatusProducer::PropagatedCall {
                        callee: operation.clone(),
                    },
                });
            }
            ResolvedExprKind::Invoke { .. } | ResolvedExprKind::Closure { .. } => {
                statuses.push(StatusSource {
                    id: StatusSourceId {
                        expression: expression.id.clone(),
                        lane: StatusLane::OperationFailure,
                    },
                    producer: StatusProducer::PropagatedCall {
                        callee: crate::hir::function_value::cleanup_call(expression)?
                            .0
                            .clone(),
                    },
                });
            }
            ResolvedExprKind::VecFieldRead { .. } => {
                statuses.push(StatusSource {
                    id: StatusSourceId {
                        expression: expression.id.clone(),
                        lane: StatusLane::OperationFailure,
                    },
                    producer: StatusProducer::PropagatedCall {
                        callee: crate::vec_field::operation_id().clone(),
                    },
                });
            }
            ResolvedExprKind::LiteralFormat { .. } => {
                statuses.push(StatusSource {
                    id: StatusSourceId {
                        expression: expression.id.clone(),
                        lane: StatusLane::OperationFailure,
                    },
                    producer: StatusProducer::PropagatedCall {
                        callee: crate::literal_format::operation_id().clone(),
                    },
                });
            }
            ResolvedExprKind::Call {
                callee, instance, ..
            } => {
                if instance.is_none()
                    && (super::super::super::deferred_commit::is_total_str_len_bytes(callee)
                        || crate::byte_ops::by_id(callee.as_str())
                            .is_some_and(|op| !op.is_fallible())
                        || crate::host_io_ops::by_id(callee.as_str()).is_some()
                        || callee.as_str() == crate::stdin_stream_ops::EOF_ID)
                {
                    // The borrowed Str byte-length read is total after HIR admission.
                    // Byte-data operations are total after HIR admission, with
                    // the bounded `bytes_set`, `bytes_set5`, and `bytes_set1_or5_from_slice` stores, whose computed
                    // element index is checked at run time. Physical allocation
                    // failure stays invariant fail-stop, not a recoverable
                    // operation status.
                    continue;
                }
                if instance.is_none()
                    && super::super::super::deferred_commit::is_infallible_vec_operation(
                        crate::vec_ops::by_id(callee.as_str()),
                    )
                {
                    continue;
                }
                if instance.is_none()
                    && super::super::super::deferred_commit::is_infallible_box_operation(callee)
                {
                    continue;
                }
                if instance.is_none()
                    && (crate::map_ops::by_id(callee.as_str()).is_some()
                        || crate::string_ops::by_id(callee.as_str()).is_some()
                        || crate::str_ops::by_id(callee.as_str()).is_some()
                        || crate::vec_ops::by_id(callee.as_str()).is_some()
                        || crate::iterator_ops::by_id(callee.as_str()).is_some()
                        || crate::list_ops::by_id(callee.as_str()).is_some()
                        || crate::box_ops::by_id(callee.as_str()).is_some()
                        || crate::byte_ops::by_id(callee.as_str()).is_some())
                {
                    // Closed String/collection/byte operations project like ordinary propagated calls.
                } else if program
                    .resolve_call_target(callee, instance.as_ref())
                    .is_none()
                {
                    return Err(replay_error(
                        function,
                        format!("status source call has unknown callee `{callee}`"),
                    ));
                }
                statuses.push(StatusSource {
                    id: StatusSourceId {
                        expression: expression.id.clone(),
                        lane: StatusLane::OperationFailure,
                    },
                    producer: StatusProducer::PropagatedCall {
                        callee: callee.clone(),
                    },
                });
            }
            ResolvedExprKind::HostCommandCall(call) => {
                if crate::command_io_ops::failure(call.operation)
                    == crate::command_io_ops::CommandIoFailure::Status
                {
                    statuses.push(StatusSource {
                        id: StatusSourceId {
                            expression: expression.id.clone(),
                            lane: StatusLane::OperationFailure,
                        },
                        producer: StatusProducer::PropagatedCall {
                            callee: DeclarationId::new(crate::command_io_ops::id(call.operation)),
                        },
                    });
                }
            }
            ResolvedExprKind::Unary {
                op: UnaryOp::Neg, ..
            } => statuses.push(checked_status(
                expression,
                super::super::super::CheckedOperation::Neg,
                vec![StatusCase::NegationOverflow],
            )),
            ResolvedExprKind::Unary {
                op: UnaryOp::Not, ..
            } => {}
            ResolvedExprKind::Binary { op, .. } => {
                let checked = match op {
                    BinaryOp::Add => Some((
                        super::super::super::CheckedOperation::Add,
                        vec![StatusCase::AddOverflow],
                    )),
                    BinaryOp::Sub => Some((
                        super::super::super::CheckedOperation::Sub,
                        vec![StatusCase::SubOverflow],
                    )),
                    BinaryOp::Mul => Some((
                        super::super::super::CheckedOperation::Mul,
                        vec![StatusCase::MulOverflow],
                    )),
                    BinaryOp::Div => Some((
                        super::super::super::CheckedOperation::Div,
                        vec![StatusCase::DivisionByZero, StatusCase::DivisionOverflow],
                    )),
                    BinaryOp::Rem => Some((
                        super::super::super::CheckedOperation::Rem,
                        vec![StatusCase::RemainderByZero, StatusCase::RemainderOverflow],
                    )),
                    BinaryOp::Eq
                    | BinaryOp::Ne
                    | BinaryOp::Lt
                    | BinaryOp::Le
                    | BinaryOp::Gt
                    | BinaryOp::Ge
                    | BinaryOp::And
                    | BinaryOp::Or => None,
                };
                if let Some((operation, cases)) = checked {
                    statuses.push(checked_status(expression, operation, cases));
                }
            }
            ResolvedExprKind::NativeRustImportCall(call) => {
                if super::super::super::native_rust::owns(expression) {
                    statuses.push(StatusSource {
                        id: StatusSourceId {
                            expression: expression.id.clone(),
                            lane: StatusLane::OperationFailure,
                        },
                        producer: StatusProducer::PropagatedCall {
                            callee: call.import.clone(),
                        },
                    });
                }
            }
            ResolvedExprKind::Block { .. }
            | ResolvedExprKind::VecConstructor { .. }
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
            | ResolvedExprKind::BorrowPlace { .. }
            | ResolvedExprKind::RowLoan { .. } => {}
        }
    }
    Ok(())
}
