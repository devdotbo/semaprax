//! Complete source type receiver with prepaid frames/results/arguments.
use super::*;
pub(super) fn resolve<E>(
    index: &DeclarationIndex,
    ty: &Type,
    owner: Option<&DeclarationId>,
    payer: &mut Payer<'_, E>,
) -> Result<Option<ResolvedType>, E> {
    enum Frame<'a> {
        Enter(&'a Type),
        Finish(DeclarationId, usize),
    }
    let mut frames = payer.vec(1)?;
    frames.push(Frame::Enter(ty));
    let mut resolved = index.nominal_drop_workspace.types();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(ty) => {
                let value = match ty {
                    Type::Function { parameters, result } => {
                        if parameters.len() > 8 {
                            return Ok(None);
                        }
                        // The independent receiver admits only direct scalar
                        // function parameters/result, checked before allocation.
                        if parameters.iter().any(|p| {
                            super::super::super::function_value::resolve::source_scalar(p).is_none()
                        }) {
                            return Ok(None);
                        }
                        let Some(result) =
                            super::super::super::function_value::resolve::source_scalar(result)
                        else {
                            return Ok(None);
                        };
                        let mut values = payer.vec(parameters.len())?;
                        for p in parameters {
                            values.push(
                                super::super::super::function_value::resolve::source_scalar(p)
                                    .unwrap(),
                            );
                        }
                        payer.bytes(size_of::<ResolvedType>() + size_of::<Box<ResolvedType>>())?;
                        ResolvedType::Function {
                            parameters: values,
                            result: Box::new(result),
                        }
                    }
                    Type::Named { name, arguments } => {
                        if arguments.is_empty() {
                            if let Some(owner) = owner {
                                if let Some(parameter) = index
                                    .type_parameters(owner)
                                    .and_then(|p| p.iter().find(|p| p.name == *name))
                                {
                                    payer.grow(&mut resolved, 1)?;
                                    resolved.push(ResolvedType::TypeParameter {
                                        owner: owner.clone(),
                                        index: parameter.index,
                                    });
                                    continue;
                                }
                            }
                        }
                        let Some(declaration) = index.type_id(name) else {
                            return Ok(None);
                        };
                        payer.grow(&mut frames, arguments.len().saturating_add(1))?;
                        frames.push(Frame::Finish(declaration.clone(), arguments.len()));
                        for argument in arguments.iter().rev() {
                            frames.push(Frame::Enter(argument));
                        }
                        continue;
                    }
                    Type::Never => return Ok(None),
                    Type::I64 => ResolvedType::I64,
                    Type::I32 => ResolvedType::I32,
                    Type::Char => ResolvedType::Char,
                    Type::U8 => ResolvedType::U8,
                    Type::Usize => ResolvedType::Usize,
                    Type::ArrayU8(n) => ResolvedType::ArrayU8(*n),
                    Type::F32 => ResolvedType::F32,
                    Type::F64 => ResolvedType::F64,
                    Type::Bool => ResolvedType::Bool,
                    Type::String => ResolvedType::String,
                    Type::Bytes => ResolvedType::Bytes,
                    Type::OnceFunction => ResolvedType::OnceFunction,
                    Type::OnceFunctionI64 => ResolvedType::OnceFunctionI64,
                    Type::OnceFunctionI64Pair => ResolvedType::OnceFunctionI64Pair,
                    Type::MutFunctionI64 => ResolvedType::MutFunctionI64,
                    Type::Str => ResolvedType::Str,
                    Type::SliceU8 => ResolvedType::SliceU8,
                    Type::StringMap => ResolvedType::StringMap,
                };
                // Acquire result storage while the root is protected by the
                // already funded construction-only recursive Drop guard.
                let value = index.nominal_drop_workspace.resolved_type(value);
                payer.grow(&mut resolved, 1)?;
                resolved.push(value.into_type());
            }
            Frame::Finish(declaration, count) => {
                let Some(split) = resolved.len().checked_sub(count) else {
                    return Ok(None);
                };
                let mut arguments = payer.vec(count)?;
                arguments.extend(resolved.drain(split..));
                // No growth is needed: draining count children frees a slot
                // except for a nullary nominal, whose growth is paid first.
                let value = index
                    .nominal_drop_workspace
                    .resolved_type(ResolvedType::Nominal {
                        declaration,
                        arguments,
                    });
                payer.grow(&mut resolved, 1)?;
                resolved.push(value.into_type());
            }
        }
    }
    Ok((resolved.len() == 1).then(|| resolved.pop().unwrap()))
}
