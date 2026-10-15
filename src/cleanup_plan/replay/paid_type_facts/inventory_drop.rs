//! Complete outer ordinary cleanup bool receiver with mandatory existing work.
//! Original ordinary callback and paid receiver remain independently compiled.
use super::inventory_metadata as metadata;
use super::*;
use crate::hir::ResolvedTypeDeclarationKind;
use std::borrow::Cow;

pub(super) fn needs_drop(
    program: &dyn crate::cleanup::ReplayContext,
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    inventory_text::current(payer)?;
    metadata::step(payer, 1)?;
    // A terminal expression has no storage. Nested types still require facts.
    if *ty == ResolvedType::Never {
        payer.bytes(0)?;
        return Ok(false);
    }
    // The root classifier retains exact hostile-cache precedence and uses the
    // existing caller payer. Only nominal roots need an outer traversal carrier.
    let first = inventory_flags::flags(program.declarations(), ty, payer)?;
    let Some(first) = first else {
        let key = inventory_raw_key::identity(ty, payer)?;
        return Err(Diagnostic::io(
            "SPX-H006",
            inventory_text::raw(payer, format_args!("type `{key}` has no cleanup facts"))?,
        ));
    };
    if !first.needs_drop {
        return Ok(false);
    }
    match ty {
        ResolvedType::Bytes
        | ResolvedType::String
        | ResolvedType::StringMap
        | ResolvedType::OnceFunction
        | ResolvedType::OnceFunctionI64
        | ResolvedType::OnceFunctionI64Pair
        | ResolvedType::TypeParameter { .. } => return Ok(true),
        ResolvedType::Str
        | ResolvedType::SliceU8
        | ResolvedType::Function { .. }
        | ResolvedType::MutFunctionI64 => return Ok(false),
        ResolvedType::Nominal { .. } => {}
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
        | ResolvedType::Bool => {
            let key = inventory_raw_key::identity(ty, payer)?;
            return Err(Diagnostic::io(
                "SPX-H006",
                inventory_text::raw(
                    payer,
                    format_args!("copy type `{key}` unexpectedly requires cleanup"),
                )?,
            ));
        }
    }
    // The first pending member is exactly this borrowed root; carry its paid
    // facts once rather than charging or classifying it a second time.
    let mut first = Some(first);
    let mut pending = inventory_flags::vector(payer)?;
    inventory_flags::push(&mut pending, Cow::Borrowed(ty), payer)?;
    let mut visited = inventory_ordered::Map::<()>::acquire(payer)?;
    loop {
        metadata::step(payer, 1)?;
        let Some(ty) = pending.pop() else {
            break;
        };
        let facts = match first.take() {
            Some(facts) => Some(facts),
            None => inventory_flags::flags(program.declarations(), ty.as_ref(), payer)?,
        };
        let Some(facts) = facts else {
            let key = inventory_raw_key::identity(ty.as_ref(), payer)?;
            return Err(Diagnostic::io(
                "SPX-H006",
                inventory_text::raw(payer, format_args!("type `{key}` has no cleanup facts"))?,
            ));
        };
        if !facts.needs_drop {
            continue;
        }
        // Full raw length-delimited structural identity, never capped classifier
        // sink text. It retains every nested generic argument and callable field.
        let identity = inventory_raw_key::identity(ty.as_ref(), payer)?;
        if visited.insert(identity, (), payer)?.is_some() {
            continue;
        }
        // The sealed stdin reader is compiler-owned and has no user-visible
        // fields, but it still owns the invocation's reusable input buffer.
        // Classify its exact nominal type as droppable so inventory and plan
        // construction retain the canonical lexical drop leaf.
        if inventory_flags::families::typed_collection(ty.as_ref(), payer)? {
            return Ok(true);
        }
        let reader = if let ResolvedType::Nominal {
            declaration,
            arguments,
        } = ty.as_ref()
        {
            inventory_text::same_bytes(
                declaration.as_str(),
                crate::stdin_stream_ops::READER_ID,
                payer,
            )? && arguments.is_empty()
        } else {
            false
        };
        if reader {
            return Ok(true);
        }
        match ty.as_ref() {
            ResolvedType::Bytes
            | ResolvedType::String
            | ResolvedType::OnceFunction
            | ResolvedType::OnceFunctionI64
            | ResolvedType::OnceFunctionI64Pair => return Ok(true),
            ResolvedType::MutFunctionI64 => {}
            ResolvedType::StringMap => return Ok(true),
            ResolvedType::Function { .. } | ResolvedType::Str | ResolvedType::SliceU8 => {}
            ResolvedType::Nominal {
                declaration,
                arguments,
            } => {
                if inventory_text::same_bytes(
                    declaration.as_str(),
                    crate::iterator_ops::ITER_ID,
                    payer,
                )? {
                    if let [element] = arguments.as_slice() {
                        if inventory_flags::families::iterator_element(
                            program.declarations(),
                            element,
                            payer,
                        )? {
                            return Ok(true);
                        }
                    }
                }
                if inventory_text::same_bytes(declaration.as_str(), crate::prelude::BOX_ID, payer)?
                {
                    if let [element] = arguments.as_slice() {
                        metadata::step(payer, 1)?;
                        if crate::box_ops::resolved_box_element_is_admitted(element) {
                            return Ok(true);
                        }
                    }
                }
                if inventory_text::same_bytes(declaration.as_str(), crate::prelude::VEC_ID, payer)?
                {
                    if let [element] = arguments.as_slice() {
                        if inventory_flags::families::vector_element(
                            program.declarations(),
                            element,
                            payer,
                        )? {
                            return Ok(true);
                        }
                    }
                }
                let mut item = None;
                for row in program.types() {
                    metadata::step(payer, 1)?;
                    if inventory_text::owner_matches(&row.id, declaration, payer)? {
                        item = Some(row);
                        break;
                    }
                }
                let Some(item) = item else {
                    return Err(Diagnostic::io(
                        "SPX-H006",
                        inventory_text::raw(
                            payer,
                            format_args!("unknown cleanup type `{declaration}`"),
                        )?,
                    ));
                };
                match &item.kind {
                    ResolvedTypeDeclarationKind::Resource { .. } => return Ok(true),
                    ResolvedTypeDeclarationKind::Record { fields }
                    | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                        inventory_flags::grow(&mut pending, fields.len(), payer)?;
                        for field in fields.iter().rev() {
                            metadata::step(payer, 1)?;
                            let resolved = crate::hir::inventory_substitute(
                                &field.ty,
                                declaration,
                                arguments,
                                &mut *payer,
                            )?;
                            let Some(resolved) = resolved else {
                                return Err(Diagnostic::io(
                                    "SPX-H006",
                                    inventory_text::raw(
                                        payer,
                                        format_args!(
                                            "type `{declaration}` cleanup substitution failed"
                                        ),
                                    )?,
                                ));
                            };
                            inventory_flags::push(&mut pending, Cow::Owned(resolved), payer)?;
                        }
                    }
                    ResolvedTypeDeclarationKind::Variant { cases } => {
                        let mut fields = Some(0usize);
                        for case in cases {
                            metadata::step(payer, 1)?;
                            fields = fields.and_then(|n| n.checked_add(case.fields.len()));
                            if fields.is_none() {
                                break;
                            }
                        }
                        let Some(fields) = fields else {
                            return Err(Diagnostic::io(
                                "SPX-H006",
                                inventory_text::raw(
                                    payer,
                                    format_args!("cleanup type traversal work overflowed"),
                                )?,
                            ));
                        };
                        inventory_flags::grow(&mut pending, fields, payer)?;
                        for case in cases.iter().rev() {
                            metadata::step(payer, 1)?;
                            for field in case.fields.iter().rev() {
                                metadata::step(payer, 1)?;
                                let resolved = crate::hir::inventory_substitute(
                                    &field.ty,
                                    declaration,
                                    arguments,
                                    &mut *payer,
                                )?;
                                let Some(resolved) = resolved else {
                                    return Err(Diagnostic::io(
                                        "SPX-H006",
                                        inventory_text::raw(
                                            payer,
                                            format_args!(
                                                "type `{declaration}` cleanup substitution failed"
                                            ),
                                        )?,
                                    ));
                                };
                                inventory_flags::push(&mut pending, Cow::Owned(resolved), payer)?;
                            }
                        }
                    }
                }
            }
            // Preserve the shape builder's unsupported-generic diagnostic.
            ResolvedType::TypeParameter { .. } => return Ok(true),
            ResolvedType::OnceFunction
            | ResolvedType::OnceFunctionI64
            | ResolvedType::OnceFunctionI64Pair
            | ResolvedType::MutFunctionI64
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
            | ResolvedType::Bool => {
                let key = inventory_raw_key::identity(ty.as_ref(), payer)?;
                return Err(Diagnostic::io(
                    "SPX-H006",
                    inventory_text::raw(
                        payer,
                        format_args!("copy type `{key}` unexpectedly requires cleanup"),
                    )?,
                ));
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests;
