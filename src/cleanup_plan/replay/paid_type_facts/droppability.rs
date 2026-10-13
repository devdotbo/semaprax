//! Complete ordinary callback receiver retained as an independent test oracle.
//! The paid child acquires its outer traversal, substitution and semantic text
//! through the caller payer; cleanup.rs retains the independent original body.
use crate::diagnostic::Diagnostic;
#[cfg(test)]
use crate::hir::TypeFlags;
use crate::hir::{ResolvedType, ResolvedTypeDeclarationKind};
use std::collections::BTreeSet;

pub(super) enum Failure {
    Physical(Diagnostic),
    Semantic(String),
}

#[cfg(test)]
pub(super) fn needs_drop(
    program: &dyn crate::cleanup::ReplayContext,
    ty: &ResolvedType,
    mut classify: impl FnMut(&ResolvedType) -> Result<Option<TypeFlags>, Diagnostic>,
) -> Result<bool, Failure> {
    let mut physical = None;
    let result = (|| -> Result<bool, String> {
        // A terminal expression has no storage. Nested types still require facts.
        if *ty == ResolvedType::Never {
            return Ok(false);
        }
        let mut pending = vec![ty.clone()];
        let mut visited = BTreeSet::new();
        while let Some(ty) = pending.pop() {
            let facts = match classify(&ty) {
                Ok(facts) => facts,
                Err(error) => {
                    physical = Some(error);
                    // Inline private sentinel: no formatting or allocation follows
                    // physical refusal, and only the held Diagnostic is published.
                    return Err(String::new());
                }
            }
            .ok_or_else(|| format!("type `{}` has no cleanup facts", ty.identity_key()))?;
            if !facts.needs_drop || !visited.insert(ty.clone()) {
                continue;
            }
            // The sealed stdin reader is compiler-owned and has no user-visible
            // fields, but it still owns the invocation's reusable input buffer.
            // Classify its exact nominal type as droppable so inventory and plan
            // construction retain the canonical lexical drop leaf.
            if crate::map_ops::is_typed_collection(&ty) || crate::stdin_stream_ops::is_reader(&ty) {
                return Ok(true);
            }
            match ty {
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
                    let item = program
                        .types()
                        .iter()
                        .find(|item| item.id == declaration)
                        .ok_or_else(|| format!("unknown cleanup type `{declaration}`"))?;
                    match &item.kind {
                        ResolvedTypeDeclarationKind::Resource { .. } => return Ok(true),
                        ResolvedTypeDeclarationKind::Record { fields }
                        | ResolvedTypeDeclarationKind::Class { fields, .. } => {
                            pending.try_reserve(fields.len()).map_err(|_| {
                                "cleanup type traversal capacity exceeds address space".to_owned()
                            })?;
                            for field in fields.iter().rev() {
                                pending.push(
                                    crate::hir::substitute_type(
                                        &field.ty,
                                        &declaration,
                                        &arguments,
                                    )
                                    .map_err(|_| {
                                        format!("type `{declaration}` cleanup substitution failed")
                                    })?,
                                );
                            }
                        }
                        ResolvedTypeDeclarationKind::Variant { cases } => {
                            let fields = cases
                                .iter()
                                .try_fold(0usize, |total, case| {
                                    total.checked_add(case.fields.len())
                                })
                                .ok_or_else(|| {
                                    "cleanup type traversal work overflowed".to_owned()
                                })?;
                            pending.try_reserve(fields).map_err(|_| {
                                "cleanup type traversal capacity exceeds address space".to_owned()
                            })?;
                            for case in cases.iter().rev() {
                                for field in case.fields.iter().rev() {
                                    pending.push(
                                        crate::hir::substitute_type(
                                            &field.ty,
                                            &declaration,
                                            &arguments,
                                        )
                                        .map_err(|_| {
                                            format!(
                                                "type `{declaration}` cleanup substitution failed"
                                            )
                                        })?,
                                    );
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
                    return Err(format!(
                        "copy type `{}` unexpectedly requires cleanup",
                        ty.identity_key()
                    ));
                }
            }
        }
        Ok(false)
    })();
    match physical {
        Some(error) => Err(Failure::Physical(error)),
        None => result.map_err(Failure::Semantic),
    }
}

#[cfg(test)]
mod helper_tests;
mod key;
#[cfg(test)]
mod key_tests;
mod paid;
#[cfg(test)]
mod root_tests;
#[cfg(test)]
mod saved_paid;
#[cfg(test)]
mod tests;
mod text;
pub(super) use paid::needs_drop as needs_drop_paid;
pub(super) use text::replay_diagnostic;

/// Test-only forwarding seam for complete key receiver allocation observations.
#[cfg(test)]
pub(super) fn oracle_key_for_test<P: crate::hir::TypeFactsPayer>(
    ty: &ResolvedType,
    payer: &mut P,
) -> Result<String, Diagnostic> {
    key::identity(ty, payer)
}

// Raw ordinary inventory text on the already-held caller payer.
pub(super) fn inventory_text<P: crate::hir::TypeFactsPayer>(
    payer: &mut P,
    message: std::fmt::Arguments<'_>,
) -> Result<String, Diagnostic> {
    text::raw(payer, message)
}
