//! Exact original key traversal with physical backing acquired by its caller.
use super::text::Storage;
use crate::hir::TypeFactsPayer as Payer;
use crate::hir::{DeclarationId, ResolvedType};

fn build<P: Payer>(ty: &ResolvedType, storage: &mut Storage<'_, P>) -> Option<String> {
    if let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    {
        if arguments.is_empty() {
            return storage.text(format_args!(
                "nominal:{}:{}:0:",
                declaration.as_str().len(),
                declaration
            ));
        }
    }
    enum Frame<'a> {
        Enter(&'a ResolvedType),
        Finish(&'a DeclarationId, usize),
        FinishFunction(usize),
    }
    storage.bytes(2 * std::mem::size_of::<Vec<()>>())?;
    let mut frames = Vec::new();
    storage.push(&mut frames, Frame::Enter(ty))?;
    let mut keys = Vec::<String>::new();
    while let Some(frame) = frames.pop() {
        match frame {
            Frame::Enter(ty) => match ty {
                ResolvedType::Function { parameters, result } => {
                    storage.grow(&mut frames, parameters.len().checked_add(2)?)?;
                    frames.push(Frame::FinishFunction(parameters.len()));
                    frames.push(Frame::Enter(result));
                    frames.extend(parameters.iter().rev().map(Frame::Enter));
                }
                ResolvedType::OnceFunction => {
                    let key = storage.literal("fn-once:bytes:i64:v1")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::OnceFunctionI64 => {
                    let key = storage.literal("fn-once:bytes+i64:i64:v2")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::OnceFunctionI64Pair => {
                    let key = storage.literal("fn-once:bytes+i64+i64:i64:v3")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::MutFunctionI64 => {
                    let key = storage.literal("fn-mut:i64+i64:i64:v1")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::Never => {
                    let key = storage.literal("never:lexical-exit:v1")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::Unit => {
                    let key = storage.literal("unit")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::I64 => {
                    let key = storage.literal("i64")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::I32 => {
                    let key = storage.literal("i32")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::Char => {
                    let key = storage.literal("char")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::U8 => {
                    let key = storage.literal("u8")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::Usize => {
                    let key = storage.literal("usize")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::ArrayU8(length) => {
                    let key = storage.text(format_args!("array:u8:{length}"))?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::F32 => {
                    let key = storage.literal("f32")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::F64 => {
                    let key = storage.literal("f64")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::Bool => {
                    let key = storage.literal("bool")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::String => {
                    let key = storage.literal("string")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::Bytes => {
                    let key = storage.literal("bytes")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::Str => {
                    let key = storage.literal("str")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::SliceU8 => {
                    let key = storage.literal("slice-u8")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::StringMap => {
                    let key = storage.literal("map:string:i64:v1")?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::TypeParameter { owner, index } => {
                    let key = storage.text(format_args!(
                        "parameter:{}:{}:{index}",
                        owner.as_str().len(),
                        owner
                    ))?;
                    storage.push(&mut keys, key)?;
                }
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                } => {
                    storage.grow(&mut frames, arguments.len().checked_add(1)?)?;
                    frames.push(Frame::Finish(declaration, arguments.len()));
                    frames.extend(arguments.iter().rev().map(Frame::Enter));
                }
            },
            Frame::FinishFunction(count) => {
                let split = keys
                    .len()
                    .checked_sub(count + 1)
                    .expect("function key children retained");
                let mut key = storage.encoded()?;
                storage.append_raw(&mut key, format_args!("function:v1:{count}:"))?;
                for child in keys.drain(split..) {
                    storage.append_raw(&mut key, format_args!("{}:{child}", child.len()))?;
                }
                storage.push(&mut keys, key.into_string())?;
            }
            Frame::Finish(declaration, count) => {
                let split = keys
                    .len()
                    .checked_sub(count)
                    .expect("type-key traversal has one result per argument");
                let mut encoded = storage.encoded()?;
                for key in keys.drain(split..) {
                    storage.append(&mut encoded, format_args!("{}:{key}", key.len()))?;
                }
                let key = storage.text(format_args!(
                    "nominal:{}:{}:{}:{}",
                    declaration.as_str().len(),
                    declaration,
                    count,
                    encoded.into_string()
                ))?;
                storage.push(&mut keys, key)?;
            }
        }
    }
    keys.pop()
}

pub(super) fn identity<P: Payer>(
    ty: &ResolvedType,
    payer: &mut P,
) -> Result<String, crate::diagnostic::Diagnostic> {
    payer.bytes(std::mem::size_of::<Storage<'_, P>>())?;
    let mut storage = Storage { payer, first: None };
    let result = build(ty, &mut storage);
    match storage.first {
        Some(error) => Err(error),
        None => result.ok_or_else(|| {
            storage
                .payer
                .construction_refusal("cleanup diagnostic key traversal failed")
        }),
    }
}
