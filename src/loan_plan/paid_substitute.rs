//! Exact iterative substitution with caller-paid frames and output backing.
//! Semantic substitution failure is inline None here: projected_field's saved
//! receiver discards that diagnostic and returns false. Physical failure is Err.
use super::*;
use crate::hir::DeclarationId;
use crate::bounded_output::cfg_reservation::CfgReservation;

pub(super) fn substitute(
    template: &ResolvedType, owner: &DeclarationId, arguments: &[ResolvedType],
    payer: &CfgReservation,
) -> Result<Option<ResolvedType>, Diagnostic> {
    build(template, Some((owner, arguments)), payer)
}

fn build(
    template: &ResolvedType, substitution: Option<(&DeclarationId, &[ResolvedType])>,
    payer: &CfgReservation,
) -> Result<Option<ResolvedType>, Diagnostic> {
    enum Frame<'a> { Enter(&'a ResolvedType), Finish(&'a DeclarationId, usize), Callable(usize) }
    let mut frames = Vec::new();
    let mut resolved = Vec::new();
    storage::push(&mut frames, Frame::Enter(template), Some(payer))?;
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(template) => match template {
                ResolvedType::Function { parameters, result } => {
                    storage::push(&mut frames, Frame::Callable(parameters.len()), Some(payer))?;
                    storage::push(&mut frames, Frame::Enter(result), Some(payer))?;
                    for parameter in parameters.iter().rev() {
                        storage::push(&mut frames, Frame::Enter(parameter), Some(payer))?;
                    }
                }
                ResolvedType::TypeParameter { owner: parameter_owner, index } => {
                    let value = if let Some((owner, arguments)) = substitution {
                        if parameter_owner != owner { return Ok(None); }
                        let Ok(index) = usize::try_from(*index) else { return Ok(None); };
                        let Some(argument) = arguments.get(index) else { return Ok(None); };
                        // Clone the argument verbatim, without substituting inside it.
                        let Some(value) = build(argument, None, payer)? else { return Ok(None); };
                        value
                    } else { template.clone() };
                    storage::push(&mut resolved, value, Some(payer))?;
                }
                ResolvedType::Nominal { declaration, arguments } => {
                    storage::push(&mut frames, Frame::Finish(declaration, arguments.len()), Some(payer))?;
                    for argument in arguments.iter().rev() {
                        storage::push(&mut frames, Frame::Enter(argument), Some(payer))?;
                    }
                }
                _ => storage::push(&mut resolved, template.clone(), Some(payer))?,
            },
            Frame::Callable(count) => {
                let Some(result) = resolved.pop() else { return Ok(None); };
                let Some(split) = resolved.len().checked_sub(count) else { return Ok(None); };
                let mut parameters = storage::vector(count, Some(payer))?;
                parameters.extend(resolved.drain(split..));
                payer.array::<ResolvedType>(1).map_err(|_| storage::marker())?;
                let value = ResolvedType::Function { parameters, result: Box::new(result) };
                storage::push(&mut resolved, value, Some(payer))?;
            }
            Frame::Finish(declaration, count) => {
                let Some(split) = resolved.len().checked_sub(count) else { return Ok(None); };
                let mut arguments = storage::vector(count, Some(payer))?;
                arguments.extend(resolved.drain(split..));
                storage::push(&mut resolved, ResolvedType::Nominal { declaration: declaration.clone(), arguments }, Some(payer))?;
            }
        }
    }
    if resolved.len() != 1 { return Ok(None); }
    Ok(resolved.pop())
}

#[cfg(test)]
#[path = "paid_substitute/tests.rs"]
mod tests;
