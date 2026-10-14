//! Complete flag fold for the ordinary inventory's discarded-layout consumer.
//! The original owned TypeFacts producer remains independently compiled. Only
//! layout serialization and layout clones are omitted; complete canonical keys,
//! generic substitution, builtin/profile checks and declaration order remain.
use super::inventory_metadata as metadata;
use super::*;
use crate::hir::{DeclarationKind, TypeFlags};
use std::borrow::Cow;
pub(super) mod families;

const OWNED: TypeFlags = TypeFlags {
    copy: false,
    contains_resource: false,
    sized: true,
    needs_drop: true,
};
const COPY: TypeFlags = TypeFlags {
    copy: true,
    contains_resource: false,
    sized: true,
    needs_drop: false,
};
fn scalar(ty: &ResolvedType) -> Option<(&'static str, Option<TypeFlags>)> {
    let (key, flags) = match ty {
        ResolvedType::Never => ("never:lexical-exit:v1", None),
        ResolvedType::Unit => ("unit", Some(COPY)),
        ResolvedType::I64 => ("i64", Some(COPY)),
        ResolvedType::I32 => ("i32", Some(COPY)),
        ResolvedType::Char => ("char", Some(COPY)),
        ResolvedType::U8 => ("u8", Some(COPY)),
        ResolvedType::Usize => ("usize", Some(COPY)),
        ResolvedType::F32 => ("f32", Some(COPY)),
        ResolvedType::F64 => ("f64", Some(COPY)),
        ResolvedType::Bool => ("bool", Some(COPY)),
        ResolvedType::String => ("string", Some(OWNED)),
        ResolvedType::Bytes => ("bytes", Some(OWNED)),
        ResolvedType::OnceFunction => ("fn-once:bytes:i64:v1", Some(OWNED)),
        ResolvedType::OnceFunctionI64 => ("fn-once:bytes+i64:i64:v2", Some(OWNED)),
        ResolvedType::OnceFunctionI64Pair => ("fn-once:bytes+i64+i64:i64:v3", Some(OWNED)),
        ResolvedType::MutFunctionI64 => (
            "fn-mut:i64+i64:i64:v1",
            Some(TypeFlags {
                copy: false,
                ..COPY
            }),
        ),
        ResolvedType::Str => (
            "str",
            Some(TypeFlags {
                copy: false,
                ..COPY
            }),
        ),
        ResolvedType::SliceU8 => (
            "slice-u8",
            Some(TypeFlags {
                copy: false,
                ..COPY
            }),
        ),
        ResolvedType::StringMap => ("map:string:i64:v1", Some(OWNED)),
        _ => return None,
    };
    Some((key, flags))
}
// Scratch acquisition and replacement backing use the same physical/work
// authority. Release visits are prepaid before each newly published slot.
pub(super) fn vector<T>(payer: &mut Payer<'_>) -> Result<Vec<T>, Diagnostic> {
    metadata::step(payer, 2)?; // header acquisition and eventual buffer release
    payer.bytes(std::mem::size_of::<Vec<T>>())?;
    Ok(Vec::new())
}
pub(super) fn grow<T>(
    items: &mut Vec<T>,
    additional: usize,
    payer: &mut Payer<'_>,
) -> Result<(), Diagnostic> {
    inventory_text::current(payer)?;
    let required = items
        .len()
        .checked_add(additional)
        .ok_or_else(|| payer.construction_refusal("inventory flag frontier length overflowed"))?;
    if required > items.capacity() {
        metadata::step(payer, items.len())?;
        if items.capacity() != 0 {
            metadata::step(payer, 1)?;
        }
        payer.grow(items, additional)?;
    }
    Ok(())
}
pub(super) fn push<T>(
    items: &mut Vec<T>,
    value: T,
    payer: &mut Payer<'_>,
) -> Result<(), Diagnostic> {
    metadata::step(payer, 2)?; // publication and eventual slot release
    grow(items, 1, payer)?;
    items.push(value);
    Ok(())
}
fn identity(ty: &ResolvedType, payer: &mut Payer<'_>) -> Result<Option<String>, Diagnostic> {
    metadata::step(payer, 1)?; // eventual canonical-key release, no refund
    crate::hir::inventory_identity(ty, payer)
}
fn literal(key: &str, payer: &mut Payer<'_>) -> Result<String, Diagnostic> {
    metadata::step(payer, 1)?;
    inventory_text::raw(payer, format_args!("{key}"))
}

pub(super) fn flags(
    index: &DeclarationIndex,
    root: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<Option<TypeFlags>, Diagnostic> {
    inventory_text::current(payer)?;
    metadata::step(payer, 1)?;
    if let Some((key, flags)) = scalar(root) {
        return Ok(metadata::cached_identity(index, key, payer)?
            .map(TypeFlags::from)
            .or(flags));
    }
    if let ResolvedType::Nominal {
        declaration,
        arguments,
    } = root
    {
        if arguments.is_empty() {
            if let Some(facts) = metadata::cached_zero_argument(index, declaration, payer)? {
                return Ok(Some(TypeFlags::from(facts)));
            }
            return compute(index, root, None, payer);
        }
    }
    match metadata::cached_compound(index, root, payer)? {
        Some(Some(facts)) => Ok(Some(TypeFlags::from(facts))),
        Some(None) => compute(index, root, None, payer),
        None => {
            let Some(key) = identity(root, payer)? else {
                return Ok(None);
            };
            if let Some(facts) = metadata::cached_identity(index, &key, payer)? {
                return Ok(Some(TypeFlags::from(facts)));
            }
            compute(index, root, Some(key), payer)
        }
    }
}

enum Frame<'a> {
    Enter(Cow<'a, ResolvedType>, Option<String>),
    Finish {
        identity: String,
        declaration: DeclarationId,
        kind: DeclarationKind,
        count: usize,
    },
}
fn children(
    index: &DeclarationIndex,
    declaration: &DeclarationId,
    kind: DeclarationKind,
    arguments: &[ResolvedType],
    payer: &mut Payer<'_>,
) -> Result<Option<Vec<ResolvedType>>, Diagnostic> {
    let mut result = vector(payer)?;
    match kind {
        DeclarationKind::Record | DeclarationKind::Class => {
            let Some(fields) = metadata::fields(index, declaration, payer)? else {
                return Ok(None);
            };
            for field in fields {
                metadata::step(payer, 1)?;
                let Some(child) = crate::hir::inventory_substitute(
                    &field.ty,
                    declaration,
                    arguments,
                    &mut *payer,
                )?
                else {
                    return Ok(None);
                };
                push(&mut result, child, payer)?;
            }
        }
        DeclarationKind::Variant => {
            let Some(cases) = metadata::cases(index, declaration, payer)? else {
                return Ok(None);
            };
            // Old flatten traverses each authored case then all its fields;
            // empty cases still receive their own paid traversal visit.
            for case in cases {
                metadata::step(payer, 1)?;
                for field in &case.fields {
                    metadata::step(payer, 1)?;
                    let Some(child) = crate::hir::inventory_substitute(
                        &field.ty,
                        declaration,
                        arguments,
                        &mut *payer,
                    )?
                    else {
                        return Ok(None);
                    };
                    push(&mut result, child, payer)?;
                }
            }
        }
        _ => return Ok(None),
    }
    Ok(Some(result))
}
fn compute(
    index: &DeclarationIndex,
    root: &ResolvedType,
    root_key: Option<String>,
    payer: &mut Payer<'_>,
) -> Result<Option<TypeFlags>, Diagnostic> {
    let mut visiting = inventory_ordered::Map::<()>::acquire(payer)?;
    let mut memo = inventory_ordered::Map::<TypeFlags>::acquire(payer)?;
    let mut frames = vector(payer)?;
    let mut results = vector(payer)?;
    // Full immutable root is borrowed. Only substituted owned children require
    // fresh trees; a temporary root never becomes retained cache authority.
    push(
        &mut frames,
        Frame::Enter(Cow::Borrowed(root), root_key),
        payer,
    )?;
    loop {
        metadata::step(payer, 1)?;
        let Some(frame) = frames.pop() else {
            break;
        };
        match frame {
            Frame::Enter(ty, supplied) => {
                let identity = match supplied {
                    Some(key) => key,
                    None => {
                        let Some(key) = identity(&ty, payer)? else {
                            return Ok(None);
                        };
                        key
                    }
                };
                if let Some(flags) = memo.get(&identity, payer)?.copied() {
                    push(&mut results, flags, payer)?;
                    continue;
                }
                if families::typed_collection(&ty, payer)? {
                    push(&mut results, OWNED, payer)?;
                    continue;
                }
                if matches!(ty.as_ref(), ResolvedType::Function { .. }) {
                    if !families::signature(&ty, payer)? {
                        return Ok(None);
                    }
                    push(&mut results, COPY, payer)?;
                    continue;
                }
                if matches!(ty.as_ref(), ResolvedType::ArrayU8(_)) {
                    push(&mut results, COPY, payer)?;
                    continue;
                }
                if let Some((_, Some(flags))) = scalar(&ty) {
                    push(&mut results, flags, payer)?;
                    continue;
                }
                let native = metadata::admission::native_owned(index, &ty, payer)?
                    || families::resource_result(index, &ty, payer)?;
                let authored = metadata::admission::authored_byte_variant(index, &ty, payer)?;
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = ty.as_ref()
                else {
                    return Ok(None);
                };
                let Some(item) = metadata::declared(index, declaration, payer)? else {
                    return Ok(None);
                };
                if families::builtin(index, declaration, arguments, payer)? {
                    memo.insert(identity, OWNED, payer)?;
                    push(&mut results, OWNED, payer)?;
                    continue;
                }
                if item.kind == DeclarationKind::Resource && arguments.is_empty() {
                    let flags = TypeFlags {
                        contains_resource: true,
                        ..OWNED
                    };
                    memo.insert(identity, flags, payer)?;
                    push(&mut results, flags, payer)?;
                    continue;
                }
                if !matches!(
                    item.kind,
                    DeclarationKind::Record | DeclarationKind::Class | DeclarationKind::Variant
                ) {
                    return Ok(None);
                }
                let Some(parameters) = metadata::parameters(index, declaration, payer)? else {
                    return Ok(None);
                };
                let compiler_byte_option = inventory_text::same_bytes(
                    declaration.as_str(),
                    crate::prelude::OPTION_ID,
                    payer,
                )? && matches!(arguments.as_slice(), [ResolvedType::U8]);
                let owned_byte =
                    families::owned_byte_prelude(declaration, arguments, payer)? || authored;
                if arguments.len() != parameters.len() {
                    return Ok(None);
                }
                if !compiler_byte_option
                    && !owned_byte
                    && !native
                    && item.kind != DeclarationKind::Record
                {
                    for argument in arguments {
                        metadata::step(payer, 1)?;
                        if !matches!(argument, ResolvedType::I64 | ResolvedType::Bool) {
                            return Ok(None);
                        }
                    }
                }
                let visiting_key = literal(&identity, payer)?;
                if visiting.insert(visiting_key, (), payer)?.is_some() {
                    return Ok(None);
                }
                let Some(children) = children(index, declaration, item.kind, arguments, payer)?
                else {
                    return Ok(None);
                };
                let additional = children.len().checked_add(1).ok_or_else(|| {
                    payer.construction_refusal("inventory flag child frontier length overflowed")
                })?;
                grow(&mut frames, additional, payer)?;
                metadata::step(payer, 2)?;
                let declaration = declaration.clone();
                push(
                    &mut frames,
                    Frame::Finish {
                        identity,
                        declaration,
                        kind: item.kind,
                        count: children.len(),
                    },
                    payer,
                )?;
                for child in children.into_iter().rev() {
                    push(&mut frames, Frame::Enter(Cow::Owned(child), None), payer)?;
                }
            }
            Frame::Finish {
                identity,
                declaration,
                kind,
                count,
            } => {
                let Some(split) = results.len().checked_sub(count) else {
                    return Ok(None);
                };
                visiting.remove(&identity, payer)?;
                let mut flags = COPY;
                let mut cursor = split;
                match kind {
                    DeclarationKind::Record | DeclarationKind::Class => {
                        let Some(fields) = metadata::fields(index, &declaration, payer)? else {
                            return Ok(None);
                        };
                        // The original record zip stops at the shorter side.
                        for _field in fields.iter().take(count) {
                            metadata::step(payer, 1)?;
                            let child = results[cursor];
                            cursor += 1;
                            flags.copy &= child.copy;
                            flags.contains_resource |= child.contains_resource;
                            flags.sized &= child.sized;
                            flags.needs_drop |= child.needs_drop;
                        }
                    }
                    DeclarationKind::Variant => {
                        let Some(cases) = metadata::cases(index, &declaration, payer)? else {
                            return Ok(None);
                        };
                        for case in cases {
                            metadata::step(payer, 1)?;
                            for _field in &case.fields {
                                metadata::step(payer, 1)?;
                                let Some(child) = results.get(cursor).copied() else {
                                    return Ok(None);
                                };
                                cursor += 1;
                                flags.copy &= child.copy;
                                flags.contains_resource |= child.contains_resource;
                                flags.sized &= child.sized;
                                flags.needs_drop |= child.needs_drop;
                            }
                        }
                    }
                    _ => return Ok(None),
                }
                memo.insert(identity, flags, payer)?;
                metadata::step(payer, count)?;
                results.truncate(split);
                push(&mut results, flags, payer)?;
            }
        }
    }
    metadata::step(payer, 1)?;
    Ok((results.len() == 1).then(|| results.pop().expect("flag result length checked")))
}

#[cfg(test)]
mod tests;
