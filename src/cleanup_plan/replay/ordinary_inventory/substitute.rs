//! Paid counterpart of the complete original type substitution receiver.
use super::super::*;
use super::storage::{array, push};
fn hir_error(message: impl Into<String>) -> Diagnostic {
    Diagnostic::io("SPX-H006", message)
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
                        return Err(hir_error(format!(
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
                            hir_error(format!("type parameter index {index} does not fit usize"))
                        })?)
                        .cloned()
                        .ok_or_else(|| {
                            hir_error(format!(
                                "type template for `{owner}` references missing parameter {index}"
                            ))
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
                let result = resolved
                    .pop()
                    .ok_or_else(|| hir_error("incomplete callable substitution"))?;
                let split = resolved
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| hir_error("incomplete callable parameters"))?;
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
                let split = resolved
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| hir_error("type substitution traversal is incomplete"))?;
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
        return Err(hir_error("type substitution traversal did not settle"));
    }
    Ok(resolved
        .pop()
        .expect("substitution result count checked above"))
}
