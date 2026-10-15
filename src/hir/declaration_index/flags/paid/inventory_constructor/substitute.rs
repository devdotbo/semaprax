//! Same complete substitution semantics with an inline semantic refusal.
//! The original classifier discarded its Diagnostic with .ok(); constructing
//! that discarded text is unnecessary. Physical refusal remains independently
//! held by Storage and cannot be mistaken for semantic None.
use super::{InventoryPayer as Payer, Storage};
use crate::hir::{DeclarationId, ResolvedType};

pub(super) fn resolve<P: Payer>(
    template: &ResolvedType,
    owner: &DeclarationId,
    arguments: &[ResolvedType],
    storage: &mut Storage<P>,
) -> Option<ResolvedType> {
    enum Frame<'a> {
        Enter(&'a ResolvedType),
        Finish(&'a DeclarationId, usize),
        Callable(usize),
    }
    storage.bytes(2 * std::mem::size_of::<Vec<()>>())?;
    let mut frames = Vec::new();
    storage.inventory_push(&mut frames, Frame::Enter(template))?;
    let mut types = Vec::new();
    while let Some(frame) = frames.pop() {
        storage.work(1, "classifier substitution frame")?;
        match frame {
            Frame::Enter(ty) => match ty {
                ResolvedType::TypeParameter {
                    owner: actual,
                    index,
                } => {
                    if !storage.owner_equal(actual, owner)? {
                        return None;
                    }
                    let argument = arguments.get(usize::try_from(*index).ok()?)?;
                    let argument = storage.inventory_clone(argument)?;
                    storage.inventory_push(&mut types, argument)?;
                }
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                } => {
                    storage.inventory_grow(&mut frames, arguments.len().checked_add(1)?)?;
                    frames.push(Frame::Finish(declaration, arguments.len()));
                    storage.work(arguments.len(), "inventory nominal child scheduling")?;
                    frames.extend(arguments.iter().rev().map(Frame::Enter));
                }
                ResolvedType::Function { parameters, result } => {
                    storage.inventory_grow(&mut frames, parameters.len().checked_add(2)?)?;
                    frames.push(Frame::Callable(parameters.len()));
                    frames.push(Frame::Enter(result));
                    storage.work(parameters.len(), "inventory callable child scheduling")?;
                    frames.extend(parameters.iter().rev().map(Frame::Enter));
                }
                leaf => {
                    let leaf = storage.inventory_clone(leaf)?;
                    storage.inventory_push(&mut types, leaf)?;
                }
            },
            Frame::Finish(declaration, count) => {
                let start = types.len().checked_sub(count)?;
                let mut arguments = Vec::new();
                storage.inventory_grow(&mut arguments, count)?;
                storage.work(count, "inventory substituted children moved")?;
                arguments.extend(types.drain(start..));
                storage.work(1, "inventory held declaration reference clone")?;
                let ty = ResolvedType::Nominal {
                    declaration: declaration.clone(),
                    arguments,
                };
                storage.inventory_push(&mut types, ty)?;
            }
            Frame::Callable(count) => {
                storage.work(1, "inventory callable result box publication")?;
                let result = storage.boxed_type(types.pop()?)?;
                let start = types.len().checked_sub(count)?;
                let mut parameters = Vec::new();
                storage.inventory_grow(&mut parameters, count)?;
                storage.work(count, "inventory substituted parameters moved")?;
                parameters.extend(types.drain(start..));
                let ty = ResolvedType::Function { parameters, result };
                storage.inventory_push(&mut types, ty)?;
            }
        }
    }
    (types.len() == 1).then(|| types.pop().expect("complete type substitution result"))
}
