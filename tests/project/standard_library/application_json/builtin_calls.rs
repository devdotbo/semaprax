//! Builtins belong to typed module expressions, not compact Project cross-file edges.
use semaprax::hir::{
    self, OwnershipMode as O, ResolvedExprKind as E, ResolvedProgram, ResolvedType as T,
};

pub(super) fn assert_operation(program: &ResolvedProgram, wanted: &str) {
    // This independently authenticates types, operation identities, loans and
    // canonical call-argument staging/cleanup before inspecting the operation.
    hir::validate(program).unwrap();
    let mut found = 0;
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
    {
        let mut pending = vec![&function.body];
        pending.extend(&function.requires);
        pending.extend(&function.ensures);
        while let Some(expression) = pending.pop() {
            match &expression.kind {
                E::Call {
                    callee,
                    type_arguments,
                    instance,
                    args,
                } => {
                    if callee.as_str() == wanted {
                        found += 1;
                        assert!(
                            instance.is_none(),
                            "builtin has no authored function instance"
                        );
                        match wanted {
                            "core.vec.clone-at" => {
                                assert_eq!(args.len(), 2);
                                assert_eq!(type_arguments.len(), 1);
                                assert_eq!(expression.ty, type_arguments[0]);
                                assert_eq!(expression.ownership, O::Own);
                                // The intrinsic's first parameter borrows an owned
                                // vector place; argument nodes retain owner identity.
                                assert_eq!(args[0].ownership, O::Own);
                                assert!(
                                    matches!(&args[0].ty, T::Nominal { declaration, arguments }
                                    if declaration.as_str() == "core.vec"
                                        && arguments.as_slice() == type_arguments.as_slice())
                                );
                                let E::Place(origin) = &args[0].kind else {
                                    panic!("clone-at must borrow the exact named vector owner");
                                };
                                let loans: Vec<_> = function
                                    .loan_plan
                                    .loans
                                    .iter()
                                    .filter(|loan| {
                                        loan.site == expression.id
                                            && matches!(
                                                loan.cause,
                                                semaprax::loan_plan::LoanCause::BorrowedCall {
                                                    argument: 0
                                                }
                                            )
                                    })
                                    .collect();
                                assert_eq!(loans.len(), 1);
                                assert_eq!(&loans[0].origin, origin);
                                let commits: Vec<_> = function
                                    .cleanup_plan
                                    .blocks
                                    .iter()
                                    .flat_map(|block| &block.transitions)
                                    .filter_map(|transition| match transition {
                                        semaprax::cleanup_plan::CleanupTransition::CallCommit {
                                            call,
                                            arguments,
                                        } if call == &expression.id => Some(arguments),
                                        _ => None,
                                    })
                                    .collect();
                                assert_eq!(commits.len(), 1);
                                assert!(
                                    commits[0].is_empty(),
                                    "copy-out transfers no source owner"
                                );
                                assert_eq!(args[1].ty, T::Usize);
                                assert_eq!(args[1].ownership, O::Value);
                            }
                            "core.vec.push" => {
                                assert_eq!(args.len(), 2);
                                assert_eq!(type_arguments.len(), 1);
                                assert_eq!(expression.ty, args[0].ty);
                                assert_eq!(expression.ownership, O::Own);
                                assert_eq!(args[0].ownership, O::Own);
                                assert_eq!(args[1].ty, type_arguments[0]);
                            }
                            "core.num.char_from_i64" => {
                                assert!(type_arguments.is_empty());
                                assert_eq!(args.len(), 1);
                                assert_eq!(args[0].ty, T::I64);
                                assert_eq!(args[0].ownership, O::Value);
                                assert_eq!(expression.ty, T::Char);
                                assert_eq!(expression.ownership, O::Value);
                            }
                            _ => panic!("operation lacks an owning signature oracle: {wanted}"),
                        }
                    }
                    pending.extend(args);
                }
                E::LiteralFormat { args, .. } | E::VecFieldRead { args, .. } => {
                    pending.extend(args)
                }
                E::Unary { value, .. } => pending.push(value),
                E::Binary { left, right, .. } => pending.extend([left.as_ref(), right.as_ref()]),
                E::If {
                    condition,
                    then_branch,
                    else_branch,
                } => pending.extend([
                    condition.as_ref(),
                    then_branch.as_ref(),
                    else_branch.as_ref(),
                ]),
                E::Block { statements, tail } => {
                    pending.push(tail);
                    for statement in statements {
                        for index in 0..statement.child_count() {
                            pending.push(statement.child(index).expect("declared statement child"));
                        }
                    }
                }
                E::ConstructRecord { fields, .. } | E::ConstructVariant { fields, .. } => {
                    pending.extend(fields.iter().map(|f| &f.value))
                }
                E::UpdateRecord { base, fields, .. } => {
                    pending.push(base);
                    pending.extend(fields.iter().map(|f| &f.value));
                }
                E::Match {
                    scrutinee, arms, ..
                } => {
                    pending.push(scrutinee);
                    for arm in arms {
                        if let Some(guard) = &arm.guard {
                            pending.push(guard);
                        }
                        pending.push(&arm.value);
                    }
                }
                E::ByteRange {
                    source, start, end, ..
                } => pending.extend([source.as_ref(), start.as_ref(), end.as_ref()]),
                E::Try { operand, .. } | E::TryOption { operand, .. } => pending.push(operand),
                E::Project { base, .. } => pending.push(base),
                E::Upcast { source } => pending.push(source),
                E::LexicalExit { value, .. } => pending.extend(value.as_deref()),
                E::Closure { captures, body, .. } => {
                    pending.push(body);
                    pending.extend(captures.iter().map(|c| &c.value));
                }
                E::Invoke { callable, args } => {
                    pending.push(callable);
                    pending.extend(args);
                }
                E::Yield { request } => pending.push(request),
                E::NativeRustImportCall(call) => pending.extend(&call.args),
                E::HostCommandCall(call) => pending.extend(&call.args),
                E::Int(_)
                | E::Int32(_)
                | E::Char(_)
                | E::Uint8(_)
                | E::Usize(_)
                | E::ArrayU8(_)
                | E::RepeatArrayU8 { .. }
                | E::Float32(_)
                | E::Float64(_)
                | E::Bool(_)
                | E::String(_)
                | E::Place(_)
                | E::BorrowPlace { .. }
                | E::FunctionReference { .. } => {}
            }
        }
    }
    assert!(
        found > 0,
        "authenticated typed closure must execute {wanted}"
    );
}
