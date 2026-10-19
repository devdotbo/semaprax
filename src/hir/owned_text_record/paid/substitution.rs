//! Funded replica of `substitute_type` for the owned-text scan. Semantic
//! failures (foreign owner, missing or oversized parameter) are `Ok(None)` so no
//! ignored diagnostic text is built; budget refusals are always `Err`.
use super::{fund, reserve};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, ResolvedType};

const SLOT: usize = std::mem::size_of::<ResolvedType>();

fn add(total: Option<usize>, bytes: Option<usize>) -> Option<usize> {
    total?.checked_add(bytes?)
}

/// The argument a parameter template selects, or `None` when the original
/// substitution would have refused it.
pub(super) fn parameter<'t, I: Copy>(
    owner: &DeclarationId,
    index: I,
    expected: &DeclarationId,
    arguments: &'t [ResolvedType],
) -> Option<&'t ResolvedType>
where
    usize: TryFrom<I>,
{
    if owner != expected {
        return None;
    }
    arguments.get(usize::try_from(index).ok()?)
}

// Exact heap bytes of `ResolvedType::clone`: one Vec backing per non-empty
// argument/parameter list and one Box per callable result.
fn clone_bytes<'t>(root: &'t ResolvedType) -> Result<Option<usize>, Diagnostic> {
    let mut total = Some(0usize);
    let mut stack: Vec<&'t ResolvedType> = Vec::new();
    let mut inline = Some(root);
    while let Some(ty) = inline.take().or_else(|| stack.pop()) {
        match ty {
            ResolvedType::Nominal { arguments, .. } if !arguments.is_empty() => {
                total = add(total, arguments.len().checked_mul(SLOT));
                reserve(&mut stack, arguments.len())?;
                for argument in arguments {
                    stack.push(argument);
                }
            }
            ResolvedType::Function { parameters, result } => {
                total = add(total, parameters.len().checked_mul(SLOT));
                total = add(total, Some(SLOT));
                reserve(&mut stack, parameters.len() + 1)?;
                for parameter in parameters {
                    stack.push(parameter);
                }
                stack.push(result);
            }
            _ => {}
        }
    }
    Ok(total)
}

fn clone_funded(ty: &ResolvedType) -> Result<ResolvedType, Diagnostic> {
    fund(Some(0))?;
    fund(clone_bytes(ty)?)?;
    Ok(ty.clone())
}

/// Funded `substitute_type`. Every traversal Vec, every result Vec/Box and every
/// cloned argument is paid before it is acquired.
pub(super) fn substitute<'t>(
    template: &'t ResolvedType,
    owner: &'t DeclarationId,
    arguments: &'t [ResolvedType],
) -> Result<Option<ResolvedType>, Diagnostic> {
    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::TextSubstitution,
    );
    #[cfg(feature = "unstable-workflow-profiling")]
    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
        crate::bounded_output::exclusive_debits::Operation::TextSubstitution,
    );
    fund(Some(0))?;
    enum Frame<'f> {
        Enter(&'f ResolvedType),
        Finish(&'f DeclarationId, usize),
        FinishCallable(usize),
    }
    let mut frames: Vec<Frame<'t>> = Vec::new();
    let mut resolved: Vec<ResolvedType> = Vec::new();
    let mut inline = Some(Frame::Enter(template));
    while let Some(frame) = inline.take().or_else(|| frames.pop()) {
        match frame {
            Frame::Enter(node) => match node {
                ResolvedType::Function { parameters, result } => {
                    reserve(&mut frames, parameters.len() + 2)?;
                    frames.push(Frame::FinishCallable(parameters.len()));
                    frames.push(Frame::Enter(result));
                    for parameter in parameters.iter().rev() {
                        frames.push(Frame::Enter(parameter));
                    }
                }
                ResolvedType::TypeParameter {
                    owner: parameter_owner,
                    index,
                } => {
                    let Some(argument) = parameter(parameter_owner, *index, owner, arguments)
                    else {
                        return Ok(None);
                    };
                    let cloned = clone_funded(argument)?;
                    reserve(&mut resolved, 1)?;
                    resolved.push(cloned);
                }
                ResolvedType::Nominal {
                    declaration,
                    arguments: nested,
                } if !nested.is_empty() => {
                    reserve(&mut frames, nested.len() + 1)?;
                    frames.push(Frame::Finish(declaration, nested.len()));
                    for argument in nested.iter().rev() {
                        frames.push(Frame::Enter(argument));
                    }
                }
                // Zero-argument nominals and leaves clone without allocating.
                ResolvedType::Nominal { .. }
                | ResolvedType::Never
                | ResolvedType::Unit
                | ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::Char
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::ArrayU8(_)
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool
                | ResolvedType::String
                | ResolvedType::OnceFunction
                | ResolvedType::OnceFunctionI64
                | ResolvedType::OnceFunctionI64Pair
                | ResolvedType::MutFunctionI64
                | ResolvedType::Bytes
                | ResolvedType::Str
                | ResolvedType::SliceU8
                | ResolvedType::StringMap => {
                    reserve(&mut resolved, 1)?;
                    resolved.push(node.clone());
                }
            },
            Frame::FinishCallable(count) => {
                let Some(result) = resolved.pop() else {
                    return Ok(None);
                };
                let Some(split) = resolved.len().checked_sub(count) else {
                    return Ok(None);
                };
                fund(count.checked_mul(SLOT))?;
                let mut parameters = Vec::with_capacity(count);
                parameters.extend(resolved.drain(split..));
                fund(Some(SLOT))?;
                let result = Box::new(result);
                reserve(&mut resolved, 1)?;
                resolved.push(ResolvedType::Function { parameters, result });
            }
            Frame::Finish(declaration, count) => {
                let Some(split) = resolved.len().checked_sub(count) else {
                    return Ok(None);
                };
                fund(count.checked_mul(SLOT))?;
                let mut nested = Vec::with_capacity(count);
                nested.extend(resolved.drain(split..));
                reserve(&mut resolved, 1)?;
                resolved.push(ResolvedType::Nominal {
                    declaration: declaration.clone(),
                    arguments: nested,
                });
            }
        }
    }
    if resolved.len() != 1 {
        return Ok(None);
    }
    Ok(resolved.pop())
}
