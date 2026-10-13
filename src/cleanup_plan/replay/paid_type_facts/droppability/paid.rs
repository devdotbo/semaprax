//! Outer receiver constructors on the caller's existing explicit payer.
use super::*;
use crate::hir::TypeFactsPayer;
use std::borrow::Cow;
pub(super) fn needs_drop<P: TypeFactsPayer>(
    program: &dyn crate::cleanup::ReplayContext,
    ty: &ResolvedType,
    payer: &mut P,
) -> Result<bool, Failure> {
    // A terminal expression has no storage. Nested types still require facts.
    if *ty == ResolvedType::Never {
        payer.bytes(0).map_err(Failure::Physical)?;
        return Ok(false);
    }
    payer
        .bytes(
            std::mem::size_of::<Vec<Cow<'_, ResolvedType>>>()
                + std::mem::size_of::<BTreeSet<Cow<'_, ResolvedType>>>(),
        )
        .map_err(Failure::Physical)?;
    let mut pending = Vec::new();
    payer.grow(&mut pending, 1).map_err(Failure::Physical)?;
    pending.push(Cow::Borrowed(ty));
    let mut visited = BTreeSet::new();
    while let Some(ty) = pending.pop() {
        let facts = program
            .declarations()
            .type_flags_paid(ty.as_ref(), &mut *payer)
            .map_err(Failure::Physical)?;
        let Some(facts) = facts else {
            let key = super::key::identity(ty.as_ref(), payer).map_err(Failure::Physical)?;
            return Err(Failure::Semantic(
                super::text::raw(payer, format_args!("type `{key}` has no cleanup facts"))
                    .map_err(Failure::Physical)?,
            ));
        };
        if !facts.needs_drop || visited.contains(&ty) {
            continue;
        }
        // The root key borrows the caller. Owned substituted keys retain their
        // full paid payload; duplicate membership does not clone discarded data.
        let retained = match &ty {
            Cow::Borrowed(ty) => Cow::Borrowed(*ty),
            Cow::Owned(ty) => Cow::Owned(payer.clone_type(ty).map_err(Failure::Physical)?),
        };
        // Conservative absent-key BTree node acquisition, not an observed
        // allocation count or a refund when an existing node has room.
        payer
            .node::<Cow<'_, ResolvedType>, ()>()
            .map_err(Failure::Physical)?;
        visited.insert(retained);
        // The sealed stdin reader is compiler-owned and has no user-visible
        // fields, but it still owns the invocation's reusable input buffer.
        // Classify its exact nominal type as droppable so inventory and plan
        // construction retain the canonical lexical drop leaf.
        if crate::map_ops::is_typed_collection(ty.as_ref())
            || crate::stdin_stream_ops::is_reader(ty.as_ref())
        {
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
                if declaration.as_str() == crate::iterator_ops::ITER_ID
                    && matches!(arguments.as_slice(), [element]
                    if crate::iterator_ops::resolved_element_is_admitted_in(
                        program.declarations(),
                        element,
                    ))
                {
                    return Ok(true);
                }
                if declaration.as_str() == crate::prelude::BOX_ID
                    && matches!(arguments.as_slice(), [element] if crate::box_ops::resolved_box_element_is_admitted(element))
                {
                    return Ok(true);
                }
                if declaration.as_str() == crate::prelude::VEC_ID
                    && arguments.len() == 1
                    && (crate::vec_ops::resolved_vec_element_is_admitted(&arguments[0])
                        || crate::hir::owned_leaf_collection::copy_or_leaf_admitted(program.declarations(), &arguments[0])
                        || crate::hir::owned_record_collection::
                            is_admitted_owned_record_collection_element(
                                program.declarations(),
                                &arguments[0],
                            ))
                {
                    return Ok(true);
                }
                let Some(item) = program.types().iter().find(|item| &item.id == declaration) else {
                    return Err(Failure::Semantic(
                        super::text::raw(
                            payer,
                            format_args!("unknown cleanup type `{declaration}`"),
                        )
                        .map_err(Failure::Physical)?,
                    ));
                };
                match &item.kind {
                    ResolvedTypeDeclarationKind::Resource { .. } => return Ok(true),
                    ResolvedTypeDeclarationKind::Record { fields }
                    | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                        payer
                            .grow(&mut pending, fields.len())
                            .map_err(Failure::Physical)?;
                        for field in fields.iter().rev() {
                            let resolved = program
                                .declarations()
                                .substitute_type_paid(
                                    &field.ty,
                                    declaration,
                                    arguments,
                                    &mut *payer,
                                )
                                .map_err(Failure::Physical)?;
                            let Some(resolved) = resolved else {
                                return Err(Failure::Semantic(
                                    super::text::raw(
                                        payer,
                                        format_args!(
                                            "type `{declaration}` cleanup substitution failed"
                                        ),
                                    )
                                    .map_err(Failure::Physical)?,
                                ));
                            };
                            pending.push(Cow::Owned(resolved));
                        }
                    }
                    ResolvedTypeDeclarationKind::Variant { cases } => {
                        let fields = cases
                            .iter()
                            .try_fold(0usize, |total, case| total.checked_add(case.fields.len()));
                        let Some(fields) = fields else {
                            return Err(Failure::Semantic(
                                super::text::raw(
                                    payer,
                                    format_args!("cleanup type traversal work overflowed"),
                                )
                                .map_err(Failure::Physical)?,
                            ));
                        };
                        payer
                            .grow(&mut pending, fields)
                            .map_err(Failure::Physical)?;
                        for case in cases.iter().rev() {
                            for field in case.fields.iter().rev() {
                                let resolved = program
                                    .declarations()
                                    .substitute_type_paid(
                                        &field.ty,
                                        declaration,
                                        arguments,
                                        &mut *payer,
                                    )
                                    .map_err(Failure::Physical)?;
                                let Some(resolved) = resolved else {
                                    return Err(Failure::Semantic(
                                        super::text::raw(
                                            payer,
                                            format_args!(
                                                "type `{declaration}` cleanup substitution failed"
                                            ),
                                        )
                                        .map_err(Failure::Physical)?,
                                    ));
                                };
                                pending.push(Cow::Owned(resolved));
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
                let key = super::key::identity(ty.as_ref(), payer).map_err(Failure::Physical)?;
                return Err(Failure::Semantic(
                    super::text::raw(
                        payer,
                        format_args!("copy type `{key}` unexpectedly requires cleanup"),
                    )
                    .map_err(Failure::Physical)?,
                ));
            }
        }
    }
    Ok(false)
}
