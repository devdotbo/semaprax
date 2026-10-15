//! Complete raw canonical structural key for visited membership only. Unlike
//! the independent classifier sink, this never truncates or suppresses child
//! bytes. Complete length-delimited type encoding is injective over all current
//! ResolvedType variants; it grants no cached facts or profile admission.
use super::*;
use crate::bounded_output::classifier_string::{ClassifierString, GrowthFailure};
use std::fmt;

struct Storage<'a, 'b> {
    payer: &'a mut Payer<'b>,
    first: Option<Diagnostic>,
}
impl Storage<'_, '_> {
    fn receive<T>(&mut self, result: Result<T, Diagnostic>) -> Option<T> {
        if self.first.is_some() {
            return None;
        }
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.first = Some(error);
                None
            }
        }
    }
    fn work(&mut self, units: usize, _phase: &'static str) -> Option<()> {
        if self.first.is_some() {
            return None;
        }
        let result = inventory_metadata::step(self.payer, units);
        self.receive(result)
    }
    fn bytes(&mut self, bytes: usize) -> Option<()> {
        if self.first.is_some() {
            return None;
        }
        let result = self.payer.bytes(bytes);
        self.receive(result)
    }
    fn inventory_grow<T>(&mut self, values: &mut Vec<T>, additional: usize) -> Option<()> {
        // Prepay future slot release before receiving any new elements. Every
        // caller schedules the declared additional elements or safely drops the
        // failed receiver; retained excess backing receives no refund.
        self.work(additional, "raw key future slot release")?;
        self.work(additional, "raw key received slot publication")?;
        let required = values.len().checked_add(additional)?;
        if required <= values.capacity() {
            return Some(());
        }
        self.work(values.len(), "raw key initialized elements copied")?;
        if values.capacity() != 0 {
            self.work(1, "raw key old backing release")?;
        }
        let result = self.payer.grow(values, additional);
        self.receive(result)
    }
    fn inventory_push<T>(&mut self, values: &mut Vec<T>, value: T) -> Option<()> {
        self.work(1, "raw key publication")?;
        self.inventory_grow(values, 1)?;
        values.push(value);
        Some(())
    }
    fn growth<T>(&mut self, result: Result<T, GrowthFailure<Diagnostic>>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(GrowthFailure::Payment(error)) => {
                self.first = Some(error);
                None
            }
            Err(failure) => {
                let message = match failure {
                    GrowthFailure::Arithmetic => "inventory raw key string capacity overflowed",
                    GrowthFailure::Allocation => "inventory raw key string allocation failed",
                    GrowthFailure::Capacity => {
                        "inventory raw key string capacity disagrees with reservation"
                    }
                    GrowthFailure::Formatter => "inventory raw key formatter failed",
                    GrowthFailure::Payment(_) => {
                        unreachable!("payment handled before message selection")
                    }
                };
                self.first = Some(self.payer.construction_refusal(message));
                None
            }
        }
    }
    fn inventory_encoded(&mut self) -> Option<ClassifierString> {
        self.work(1, "raw key future string release")?;
        let result = inventory_encoded::acquire(self.payer);
        self.growth(result)
    }
    fn inventory_append(
        &mut self,
        output: &mut ClassifierString,
        arguments: fmt::Arguments<'_>,
    ) -> Option<()> {
        if self.first.is_some() {
            return None;
        }
        let result = inventory_encoded::raw(output, arguments, self.payer);
        self.growth(result)
    }
    fn inventory_append_raw(
        &mut self,
        output: &mut ClassifierString,
        arguments: fmt::Arguments<'_>,
    ) -> Option<()> {
        self.inventory_append(output, arguments)
    }
    fn inventory_append_strict(
        &mut self,
        output: &mut ClassifierString,
        arguments: fmt::Arguments<'_>,
    ) -> Option<()> {
        self.inventory_append(output, arguments)
    }
    fn inventory_text(&mut self, arguments: fmt::Arguments<'_>) -> Option<String> {
        let mut output = self.inventory_encoded()?;
        self.inventory_append(&mut output, arguments)?;
        Some(output.into_string())
    }
    fn inventory_literal(&mut self, value: &str) -> Option<String> {
        self.inventory_text(format_args!("{value}"))
    }
}
pub(super) fn identity(ty: &ResolvedType, payer: &mut Payer<'_>) -> Result<String, Diagnostic> {
    inventory_text::current(payer)?;
    inventory_metadata::step(payer, 3)?; // carrier acquisition and both future scratch releases
    payer.bytes(std::mem::size_of::<Storage<'_, '_>>())?;
    let mut storage = Storage { payer, first: None };
    let value = encode(ty, &mut storage);
    if let Some(error) = storage.first {
        return Err(error);
    }
    value.ok_or_else(|| {
        storage
            .payer
            .construction_refusal("inventory raw structural key traversal failed")
    })
}
fn encode(ty: &ResolvedType, storage: &mut Storage<'_, '_>) -> Option<String> {
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

#[cfg(test)]
mod tests;
