//! Funded borrowed traversal for the owned-text record profile.
//! The unfunded classifiers in the parent module remain the owned receivers.
//! Here every Vec backing, tree node and substitution result is paid before it
//! exists, and each request authenticates the ledger with a zero-byte reservation.
use crate::diagnostic::Diagnostic;
use crate::hir::*;
use std::borrow::Cow;
use std::collections::BTreeSet;

mod substitution;
#[cfg(test)]
pub(crate) mod tests;

fn budget() -> Diagnostic {
    hir_error_static("owned-text profile scan exceeds the active validation budget")
}

pub(crate) fn outside_profile() -> Diagnostic {
    hir_error_static("String-bearing executable record is outside the bounded owned-text profile")
}

// A zero-byte reservation authenticates the active ledger: once any reservation
// was refused it keeps refusing, so even a warm memo hit cannot succeed after it.
pub(crate) fn fund(bytes: Option<usize>) -> Result<(), Diagnostic> {
    match bytes {
        Some(bytes) if crate::bounded_output::reserve_active_required(bytes) => Ok(()),
        Some(_) => Err(budget()),
        None => {
            crate::bounded_output::refuse_active_required(usize::MAX);
            Err(budget())
        }
    }
}

// Vec growth is paid at its full new capacity before that allocation exists.
pub(crate) fn reserve<T>(items: &mut Vec<T>, additional: usize) -> Result<(), Diagnostic> {
    fund(Some(0))?;
    let Some(needed) = items.len().checked_add(additional) else {
        return fund(None);
    };
    if needed > items.capacity() {
        let Some(capacity) = items
            .capacity()
            .checked_mul(2)
            .map(|capacity| capacity.max(needed).max(4))
        else {
            return fund(None);
        };
        fund(capacity.checked_mul(std::mem::size_of::<T>()))?;
        if items.try_reserve_exact(capacity - items.len()).is_err() || items.capacity() != capacity
        {
            crate::bounded_output::refuse_active_required(usize::MAX);
            return Err(budget());
        }
    }
    Ok(())
}

/// Funded `owned_text_record::admitted`: same frames, order, limits and cycle rule.
pub(crate) fn admitted<'a>(
    root: &'a ResolvedType,
    declarations: &'a DeclarationIndex,
) -> Result<bool, Diagnostic> {
    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::TextRuntime,
    );
    #[cfg(feature = "unstable-workflow-profiling")]
    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
        crate::bounded_output::exclusive_debits::Operation::TextRuntime,
    );
    fund(Some(0))?;
    if !matches!(root, ResolvedType::Nominal { .. }) {
        return Ok(false);
    }
    enum Frame<'f> {
        Enter(&'f ResolvedType, usize),
        Leave,
    }
    let mut pending: Vec<Frame<'a>> = Vec::new();
    let mut active: Vec<&'a ResolvedType> = Vec::new();
    let mut fields = 0usize;
    let mut leaves = 0usize;
    let mut text = false;
    let mut inline = Some(Frame::Enter(root, 1));
    while let Some(frame) = inline.take().or_else(|| pending.pop()) {
        match frame {
            Frame::Enter(ty, _) if crate::map_ops::is_collection(ty) => {
                text = true;
                leaves += 1;
            }
            Frame::Enter(ResolvedType::String, _) => {
                text = true;
                leaves += 1;
            }
            Frame::Enter(ResolvedType::Bytes, _) => {
                leaves += 1;
            }
            Frame::Enter(
                ResolvedType::I64
                | ResolvedType::I32
                | ResolvedType::U8
                | ResolvedType::Usize
                | ResolvedType::Char
                | ResolvedType::F32
                | ResolvedType::F64
                | ResolvedType::Bool,
                _,
            ) => {}
            Frame::Enter(ty @ ResolvedType::Nominal { .. }, depth) => {
                if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH {
                    return Ok(false);
                }
                let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = ty
                else {
                    unreachable!()
                };
                if declarations.has_record_invariant(declaration)
                    || !arguments.is_empty()
                    || declarations
                        .type_parameters(declaration)
                        .is_none_or(|p| !p.is_empty())
                    || declarations
                        .declaration(declaration)
                        .is_none_or(|d| d.kind != DeclarationKind::Record)
                {
                    return Ok(false);
                }
                let Some(declared) = declarations.record_fields(declaration) else {
                    return Ok(false);
                };
                // Same complete structural identity; only the active path
                // participates in cycle detection, in the original order.
                if active.contains(&ty) {
                    return Ok(false);
                }
                reserve(&mut active, 1)?;
                active.push(ty);
                fields += declared.len();
                if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return Ok(false);
                }
                reserve(&mut pending, declared.len() + 1)?;
                pending.push(Frame::Leave);
                for field in declared.iter().rev() {
                    pending.push(Frame::Enter(&field.ty, depth + 1));
                }
            }
            Frame::Leave => {
                active.pop();
            }
            Frame::Enter(_, _) => return Ok(false),
        }
        if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
            return Ok(false);
        }
    }
    Ok(text)
}

/// Funded `owned_text_record::contains_string`. The visited set keeps the full
/// structural `Cow` equality; only expansions that can push children enter it.
pub(crate) fn contains_string<'a>(
    root: &'a ResolvedType,
    declarations: &'a DeclarationIndex,
) -> Result<bool, Diagnostic> {
    #[cfg(test)]
    let _operation = crate::bounded_output::stage_debits::operation_guard(
        crate::bounded_output::stage_debits::Operation::TextContains,
    );
    #[cfg(feature = "unstable-workflow-profiling")]
    let _exclusive_operation = crate::bounded_output::exclusive_debits::operation_guard(
        crate::bounded_output::exclusive_debits::Operation::TextContains,
    );
    fund(Some(0))?;
    if !matches!(root, ResolvedType::Nominal { declaration, .. }
        if declarations.declaration(declaration).is_some_and(|item| item.kind == DeclarationKind::Record))
    {
        return Ok(false);
    }
    let mut pending: Vec<Cow<'a, ResolvedType>> = Vec::new();
    let mut visited: BTreeSet<Cow<'a, ResolvedType>> = BTreeSet::new();
    let mut inline = Some(Cow::Borrowed(root));
    while let Some(ty) = inline.take().or_else(|| pending.pop()) {
        let node: &ResolvedType = &ty;
        if node == &ResolvedType::String || crate::map_ops::is_collection(node) {
            return Ok(true);
        }
        // Revisiting a type without record children pushes nothing, so only
        // expanding types need a visited entry.
        let fields = match node {
            ResolvedType::Nominal { declaration, .. } => declarations
                .record_fields(declaration)
                .filter(|fields| !fields.is_empty()),
            _ => None,
        };
        let Some(fields) = fields else {
            continue;
        };
        if visited.contains(&ty) {
            continue;
        }
        // A borrowed parent lends its arguments to bare parameter fields.
        let borrowed: Option<&'a ResolvedType> = match &ty {
            Cow::Borrowed(parent) => Some(*parent),
            Cow::Owned(_) => None,
        };
        let shared: Option<&'a [ResolvedType]> = match borrowed {
            Some(ResolvedType::Nominal { arguments, .. }) => Some(arguments.as_slice()),
            _ => None,
        };
        if let ResolvedType::Nominal {
            declaration,
            arguments,
        } = node
        {
            for field in fields {
                // Leaf and zero-argument nominal substitution is identity.
                // Parameter and compound templates retain the exact fallible
                // substitution route, including foreign-owner refusal.
                let identity = match &field.ty {
                    ResolvedType::Nominal {
                        arguments: nested, ..
                    } => nested.is_empty(),
                    ResolvedType::Function { .. } | ResolvedType::TypeParameter { .. } => false,
                    ResolvedType::Never
                    | ResolvedType::OnceFunction
                    | ResolvedType::OnceFunctionI64
                    | ResolvedType::OnceFunctionI64Pair
                    | ResolvedType::MutFunctionI64
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
                    | ResolvedType::Bytes
                    | ResolvedType::Str
                    | ResolvedType::SliceU8
                    | ResolvedType::StringMap => true,
                };
                let next: Option<Cow<'a, ResolvedType>> = if identity {
                    Some(Cow::Borrowed(&field.ty))
                } else {
                    match (&field.ty, shared) {
                        (ResolvedType::TypeParameter { owner, index }, Some(shared)) => {
                            substitution::parameter(owner, *index, declaration, shared)
                                .map(Cow::Borrowed)
                        }
                        _ => substitution::substitute(&field.ty, declaration, arguments)?
                            .map(Cow::Owned),
                    }
                };
                if let Some(next) = next {
                    // Reuse the already-live inline slot for the last child.
                    // Spill earlier children in authored order, so the same
                    // last-authored child is still visited first. A singleton
                    // borrows its field without acquiring any pending backing.
                    if inline.is_some() {
                        reserve(&mut pending, 1)?;
                        pending.push(inline.take().expect("inline child checked"));
                    }
                    inline = Some(next);
                }
            }
        }
        // Move owned substitutions once; borrowed roots/fields stay references.
        fund(crate::cleanup_plan::replay_btree_node_upper::<
            Cow<'a, ResolvedType>,
            (),
        >())?;
        visited.insert(ty);
    }
    Ok(false)
}

/// The owned collection receiver replayed by the funded early shape walk, which
/// has identical gates, order, field-owner checks and limits.
pub(crate) fn collection_admitted(
    root: &ResolvedType,
    declarations: &DeclarationIndex,
) -> Result<bool, Diagnostic> {
    fund(Some(0))?;
    crate::hir::declared_collection::funded_early::record_admitted(root, declarations)
}

/// Ordinary internal record runtime; the collection profile is consulted only
/// after text admission fails, exactly as in the owned receiver.
pub(crate) fn runtime_admitted<'a>(
    root: &'a ResolvedType,
    declarations: &'a DeclarationIndex,
) -> Result<bool, Diagnostic> {
    fund(Some(0))?;
    if admitted(root, declarations)? {
        return Ok(true);
    }
    collection_admitted(root, declarations)
}

pub(crate) fn validate_use<'a>(
    root: &'a ResolvedType,
    declarations: &'a DeclarationIndex,
) -> Result<(), Diagnostic> {
    fund(Some(0))?;
    if contains_string(root, declarations)? && !runtime_admitted(root, declarations)? {
        return Err(outside_profile());
    }
    Ok(())
}
