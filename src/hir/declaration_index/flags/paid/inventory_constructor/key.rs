//! Exact original key traversal with physical backing acquired by its caller.
use super::{InventoryPayer as Payer, Storage};
use crate::hir::{DeclarationId, ResolvedType};

pub(super) fn identity<P: Payer>(ty: &ResolvedType, storage: &mut Storage<P>) -> Option<String> {
    if let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    {
        if arguments.is_empty() {
            storage.work(1, "classifier nominal key receive")?;
            return storage.inventory_text(format_args!(
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
    storage.inventory_push(&mut frames, Frame::Enter(ty))?;
    let mut keys = Vec::<String>::new();
    while let Some(frame) = frames.pop() {
        storage.work(1, "classifier key frame")?;
        match frame {
            Frame::Enter(ty) => match ty {
                ResolvedType::Function { parameters, result } => {
                    storage.inventory_grow(&mut frames, parameters.len().checked_add(2)?)?;
                    frames.push(Frame::FinishFunction(parameters.len()));
                    frames.push(Frame::Enter(result));
                    storage.work(parameters.len(), "inventory callable child scheduling")?;
                    frames.extend(parameters.iter().rev().map(Frame::Enter));
                }
                ResolvedType::OnceFunction => {
                    let key = storage.inventory_literal("fn-once:bytes:i64:v1")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::OnceFunctionI64 => {
                    let key = storage.inventory_literal("fn-once:bytes+i64:i64:v2")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::OnceFunctionI64Pair => {
                    let key = storage.inventory_literal("fn-once:bytes+i64+i64:i64:v3")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::MutFunctionI64 => {
                    let key = storage.inventory_literal("fn-mut:i64+i64:i64:v1")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::Never => {
                    let key = storage.inventory_literal("never:lexical-exit:v1")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::Unit => {
                    let key = storage.inventory_literal("unit")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::I64 => {
                    let key = storage.inventory_literal("i64")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::I32 => {
                    let key = storage.inventory_literal("i32")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::Char => {
                    let key = storage.inventory_literal("char")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::U8 => {
                    let key = storage.inventory_literal("u8")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::Usize => {
                    let key = storage.inventory_literal("usize")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::ArrayU8(length) => {
                    let key = storage.inventory_text(format_args!("array:u8:{length}"))?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::F32 => {
                    let key = storage.inventory_literal("f32")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::F64 => {
                    let key = storage.inventory_literal("f64")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::Bool => {
                    let key = storage.inventory_literal("bool")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::String => {
                    let key = storage.inventory_literal("string")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::Bytes => {
                    let key = storage.inventory_literal("bytes")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::Str => {
                    let key = storage.inventory_literal("str")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::SliceU8 => {
                    let key = storage.inventory_literal("slice-u8")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::StringMap => {
                    let key = storage.inventory_literal("map:string:i64:v1")?;
                    storage.inventory_push(&mut keys, key)?;
                }
                ResolvedType::TypeParameter { owner, index } => {
                    let key = storage.inventory_text(format_args!(
                        "parameter:{}:{}:{index}",
                        owner.as_str().len(),
                        owner
                    ))?;
                    storage.inventory_push(&mut keys, key)?;
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
            },
            Frame::FinishFunction(count) => {
                let split = keys
                    .len()
                    .checked_sub(count + 1)
                    .expect("function key children retained");
                let mut key = storage.inventory_encoded()?;
                storage.inventory_append_strict(&mut key, format_args!("function:v1:{count}:"))?;
                for child in keys.drain(split..) {
                    storage.work(1, "classifier key child receive")?;
                    storage
                        .inventory_append_raw(&mut key, format_args!("{}:{child}", child.len()))?;
                }
                storage.inventory_push(&mut keys, key.into_string())?;
            }
            Frame::Finish(declaration, count) => {
                let split = keys
                    .len()
                    .checked_sub(count)
                    .expect("type-key traversal has one result per argument");
                let mut encoded = storage.inventory_encoded()?;
                for key in keys.drain(split..) {
                    storage.work(1, "classifier key child receive")?;
                    storage.inventory_append(&mut encoded, format_args!("{}:{key}", key.len()))?;
                }
                let key = storage.inventory_text(format_args!(
                    "nominal:{}:{}:{}:{}",
                    declaration.as_str().len(),
                    declaration,
                    count,
                    encoded.into_string()
                ))?;
                storage.inventory_push(&mut keys, key)?;
            }
        }
    }
    keys.pop()
}
