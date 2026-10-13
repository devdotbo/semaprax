//! Paid counterpart of the complete original type substitution receiver.
use super::super::*;
use super::storage::{array, push};
fn hir_error_old(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
}

/// Immediate borrowed text; ordinary construction delegates to its complete
/// original receiver. Only a matching explicit account may fund raw H006 text.
fn hir_error(
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
    message: std::fmt::Arguments<'_>,
) -> Diagnostic {
    #[cfg(test)]
    if let Some(lease) = budget.oracle_lease() {
        if !matches!(allocation::authority_for_function(function), Ok(Some(held)) if held == lease)
        {
            lease.fail_physical("raw diagnostic payer differs from its live invocation");
            return Diagnostic::io_static(
                "SPX-H006",
                "oracle invocation authority refused a raw diagnostic",
            );
        }
        return match allocation::raw_error_funded(lease, message) {
            Ok(error) => error,
            Err(refusal) => refusal.into_diagnostic(),
        };
    }
    let _ = (function, budget);
    hir_error_old(std::fmt::format(message))
}

pub(in crate::cleanup_plan::replay) fn substitute_type(
    template: &ResolvedType,
    owner: &DeclarationId,
    arguments: &[ResolvedType],
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<ResolvedType, Diagnostic> {
    enum Frame<'a> {
        Enter(&'a ResolvedType),
        Finish(&'a DeclarationId, usize),
        FinishCallable(usize),
    }
    let mut frames = Vec::new();
    budget.reserve_vec(function, &mut frames, 1, "shape substitution frames")?;
    frames.push(Frame::Enter(template));
    let mut resolved = Vec::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(template) => match template {
                ResolvedType::Function { parameters, result } => {
                    budget.reserve_vec(
                        function,
                        &mut frames,
                        parameters.len().saturating_add(2),
                        "shape callable substitution frames",
                    )?;
                    frames.push(Frame::FinishCallable(parameters.len()));
                    frames.push(Frame::Enter(result));
                    frames.extend(parameters.iter().rev().map(Frame::Enter));
                }
                ResolvedType::Never => push(
                    &mut resolved,
                    ResolvedType::Never,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::Unit => push(
                    &mut resolved,
                    ResolvedType::Unit,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::I64 => push(
                    &mut resolved,
                    ResolvedType::I64,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::I32 => push(
                    &mut resolved,
                    ResolvedType::I32,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::Char => push(
                    &mut resolved,
                    ResolvedType::Char,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::U8 => push(
                    &mut resolved,
                    ResolvedType::U8,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::Usize => push(
                    &mut resolved,
                    ResolvedType::Usize,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::ArrayU8(length) => push(
                    &mut resolved,
                    ResolvedType::ArrayU8(*length),
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::F32 => push(
                    &mut resolved,
                    ResolvedType::F32,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::F64 => push(
                    &mut resolved,
                    ResolvedType::F64,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::Bool => push(
                    &mut resolved,
                    ResolvedType::Bool,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::String => push(
                    &mut resolved,
                    ResolvedType::String,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::OnceFunction => push(
                    &mut resolved,
                    ResolvedType::OnceFunction,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::OnceFunctionI64 => push(
                    &mut resolved,
                    ResolvedType::OnceFunctionI64,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::MutFunctionI64 => push(
                    &mut resolved,
                    ResolvedType::MutFunctionI64,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::OnceFunctionI64Pair => push(
                    &mut resolved,
                    ResolvedType::OnceFunctionI64Pair,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::Bytes => push(
                    &mut resolved,
                    ResolvedType::Bytes,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::Str => push(
                    &mut resolved,
                    ResolvedType::Str,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::SliceU8 => push(
                    &mut resolved,
                    ResolvedType::SliceU8,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::StringMap => push(
                    &mut resolved,
                    ResolvedType::StringMap,
                    function,
                    budget,
                    "shape substitution results",
                )?,
                ResolvedType::TypeParameter {
                    owner: parameter_owner,
                    index,
                } => {
                    if parameter_owner != owner {
                        return Err(hir_error(function, budget, format_args!(
                            "type template for `{owner}` contains foreign parameter owner `{parameter_owner}`"
                        )));
                    }
                    if let Some(argument) = usize::try_from(*index)
                        .ok()
                        .and_then(|index| arguments.get(index))
                    {
                        budget.reserve_clone(
                            function,
                            argument,
                            "shape substituted parameter payload",
                        )?;
                    }
                    let argument = arguments
                        .get(usize::try_from(*index).map_err(|_| {
                            hir_error(
                                function,
                                budget,
                                format_args!("type parameter index {index} does not fit usize"),
                            )
                        })?)
                        .cloned()
                        .ok_or_else(|| {
                            hir_error(
                                function,
                                budget,
                                format_args!(
                                "type template for `{owner}` references missing parameter {index}"
                            ),
                            )
                        })?;
                    push(
                        &mut resolved,
                        argument,
                        function,
                        budget,
                        "shape substitution results",
                    )?;
                }
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                } => {
                    budget.reserve_vec(
                        function,
                        &mut frames,
                        arguments.len().saturating_add(1),
                        "shape nominal substitution frames",
                    )?;
                    frames.push(Frame::Finish(declaration, arguments.len()));
                    frames.extend(arguments.iter().rev().map(Frame::Enter));
                }
            },
            Frame::FinishCallable(count) => {
                let result = resolved.pop().ok_or_else(|| {
                    hir_error(
                        function,
                        budget,
                        format_args!("incomplete callable substitution"),
                    )
                })?;
                let split = resolved.len().checked_sub(count).ok_or_else(|| {
                    hir_error(
                        function,
                        budget,
                        format_args!("incomplete callable parameters"),
                    )
                })?;
                let mut parameters =
                    array::<ResolvedType>(function, count, budget, "shape callable parameters")?;
                parameters.extend(resolved.drain(split..));
                budget.reserve_bytes(
                    function,
                    std::mem::size_of::<ResolvedType>(),
                    "shape callable result box",
                )?;
                push(
                    &mut resolved,
                    ResolvedType::Function {
                        parameters,
                        result: Box::new(result),
                    },
                    function,
                    budget,
                    "shape substitution results",
                )?;
            }
            Frame::Finish(declaration, count) => {
                let split = resolved.len().checked_sub(count).ok_or_else(|| {
                    hir_error(
                        function,
                        budget,
                        format_args!("type substitution traversal is incomplete"),
                    )
                })?;
                let mut nested =
                    array::<ResolvedType>(function, count, budget, "shape nominal arguments")?;
                nested.extend(resolved.drain(split..));
                push(
                    &mut resolved,
                    ResolvedType::Nominal {
                        declaration: declaration.clone(),
                        arguments: nested,
                    },
                    function,
                    budget,
                    "shape substitution results",
                )?;
            }
        }
    }
    if resolved.len() != 1 {
        return Err(hir_error(
            function,
            budget,
            format_args!("type substitution traversal did not settle"),
        ));
    }
    Ok(resolved
        .pop()
        .expect("substitution result count checked above"))
}

#[cfg(test)]
mod oracle_error_tests {
    use super::*;
    use crate::cleanup_plan::replay::allocation::{enter, OracleScope};

    #[test]
    fn raw_substitution_error_keeps_complete_old_diagnostic_and_is_paid_through_drop() {
        let checked = crate::check(
            r#"module raw.oracle; @id("main") fn main()->i64 {7}"#,
            "raw-oracle.spx",
        )
        .unwrap();
        let program = crate::hir::resolve(&checked).unwrap();
        let function = &program.functions[0];
        let owner = &function.id;
        let template = ResolvedType::TypeParameter {
            owner: owner.clone(),
            index: 7,
        };
        let original = hir_error_old(format!(
            "type template for `{owner}` references missing parameter 7"
        ));
        let run = |allowance| {
            let scope = OracleScope::open(allowance).unwrap();
            let lease = scope.lease();
            let ((same, metrics), refused, active) =
                crate::bounded_output::with_limit_usage(0, || {
                    crate::test_allocation_observer::measure(|| {
                        let invocation = enter(&program, function, lease).unwrap();
                        let mut budget =
                            ReplayBudget::with_skeleton_limit_oracle(MAX_REPLAY_WORK_UNITS, lease);
                        let error = substitute_type(&template, owner, &[], function, &mut budget)
                            .unwrap_err();
                        let same = error == original;
                        drop(error);
                        drop(budget);
                        assert_eq!(invocation.exit(), Ok(()));
                        same
                    })
                });
            assert!(!refused);
            assert_eq!(active, 0);
            let receipt = scope.finish();
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert!(metrics.allocated <= receipt.charged);
            (same, receipt)
        };
        let (same, baseline) = run(1 << 20);
        assert!(same, "all six unprefixed original diagnostic fields");
        assert!(baseline.charged > original.message.len());
        assert!(baseline.first_refusal.is_none());
        let (same, exact) = run(baseline.charged);
        assert!(same);
        assert_eq!(exact.charged, baseline.charged);
        for allowance in [baseline.charged - 1, 0] {
            let (same, receipt) = run(allowance);
            assert!(
                !same,
                "physical refusal cannot publish the semantic diagnostic"
            );
            assert!(receipt.first_refusal.is_some());
        }
    }
}
