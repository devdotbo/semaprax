//! Full argument clone without a bounded census or a second owned key.
use super::*;

// Final child counts are already known. Match the ordinary Vec clone's exact
// backing rather than using the dynamic scratch frontier's growth policy.
pub(super) fn exact<T>(count: usize, payer: &mut Payer<'_>) -> Result<Vec<T>, Diagnostic> {
    inventory_text::current(payer)?;
    payer
        .budget
        .reserve_array::<T>(payer.function, count, "inventory fixed type children")?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(count)
        .map_err(|_| payer.construction_refusal("inventory type child allocation failed"))?;
    if values.capacity() != count {
        return Err(
            payer.construction_refusal("inventory type child capacity disagrees with reservation")
        );
    }
    Ok(values)
}

pub(super) fn owns_backing(ty: &ResolvedType) -> bool {
    match ty {
        ResolvedType::Nominal { arguments, .. } => !arguments.is_empty(),
        ResolvedType::Function { .. } => true,
        ResolvedType::Never
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
        | ResolvedType::StringMap
        | ResolvedType::TypeParameter { .. } => false,
    }
}

pub(super) fn clone_type(
    root: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<ResolvedType, Diagnostic> {
    let function = payer.function;
    inventory_text::current(payer)?;
    payer
        .budget
        .charge(function, 1, "inventory cloned type root")?;
    if !owns_backing(root) {
        payer.bytes(0)?;
        return Ok(root.clone());
    }
    enum Frame<'a> {
        Enter(&'a ResolvedType),
        Nominal(&'a DeclarationId, usize),
        Callable(usize),
    }
    let mut pending = Vec::new();
    let mut values = Vec::new();
    payer.grow(&mut pending, 1)?;
    pending.push(Frame::Enter(root));
    while let Some(frame) = pending.pop() {
        payer
            .budget
            .charge(function, 1, "inventory argument clone frame")?;
        match frame {
            Frame::Enter(ty) => match ty {
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                } => {
                    let additional = arguments.len().checked_add(1).ok_or_else(|| {
                        payer.construction_refusal("inventory clone frame count overflowed")
                    })?;
                    payer.grow(&mut pending, additional)?;
                    pending.push(Frame::Nominal(declaration, arguments.len()));
                    payer.budget.charge(
                        function,
                        arguments.len(),
                        "inventory clone child scheduling",
                    )?;
                    pending.extend(arguments.iter().rev().map(Frame::Enter));
                }
                ResolvedType::Function { parameters, result } => {
                    let additional = parameters.len().checked_add(2).ok_or_else(|| {
                        payer.construction_refusal("inventory clone frame count overflowed")
                    })?;
                    payer.grow(&mut pending, additional)?;
                    pending.push(Frame::Callable(parameters.len()));
                    pending.push(Frame::Enter(result));
                    payer.budget.charge(
                        function,
                        parameters.len(),
                        "inventory clone callable child scheduling",
                    )?;
                    pending.extend(parameters.iter().rev().map(Frame::Enter));
                }
                leaf @ (ResolvedType::Never
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
                | ResolvedType::StringMap
                | ResolvedType::TypeParameter { .. }) => {
                    payer.grow(&mut values, 1)?;
                    values.push(leaf.clone());
                }
            },
            Frame::Nominal(declaration, count) => {
                let start = values
                    .len()
                    .checked_sub(count)
                    .ok_or_else(|| payer.construction_refusal("inventory clone children absent"))?;
                let mut arguments = exact(count, payer)?;
                arguments.extend(values.drain(start..));
                payer.grow(&mut values, 1)?;
                values.push(ResolvedType::Nominal {
                    declaration: declaration.clone(),
                    arguments,
                });
            }
            Frame::Callable(count) => {
                let result = values
                    .pop()
                    .ok_or_else(|| payer.construction_refusal("inventory clone result absent"))?;
                let start = values.len().checked_sub(count).ok_or_else(|| {
                    payer.construction_refusal("inventory clone parameters absent")
                })?;
                let mut parameters = exact(count, payer)?;
                parameters.extend(values.drain(start..));
                let result = payer.boxed_type(result)?;
                payer.grow(&mut values, 1)?;
                values.push(ResolvedType::Function { parameters, result });
            }
        }
    }
    if values.len() != 1 {
        return Err(payer.construction_refusal("inventory clone did not settle"));
    }
    Ok(values.pop().expect("inventory clone result count checked"))
}
