//! Funded early admission predicates for the declaration-only collection scan.
//! Each replays its unchanged original receiver exactly; only constructors are paid.
use super::{fund, reserve};
use crate::diagnostic::Diagnostic;
use crate::hir::{DeclarationId, DeclarationIndex, DeclarationKind, IdentityOrigin, ResolvedType};
use std::collections::BTreeSet;

// A node is paid at the tree upper bound before the insertion that may allocate it.
fn insert_set<K: Ord>(set: &mut BTreeSet<K>, key: K) -> Result<bool, Diagnostic> {
    if set.contains(&key) {
        return Ok(false);
    }
    fund(crate::cleanup_plan::replay_btree_node_upper::<K, ()>())?;
    Ok(set.insert(key))
}
// Existing exact byte-stream probe compares declaration.as_str() + "#invariant"
// against the same declaration map and arbitrary stored declaration presence.
// It constructs neither formatted String nor temporary DeclarationId backing.
pub(crate) fn invariant_present(
    index: &DeclarationIndex,
    declaration: &DeclarationId,
) -> Result<bool, Diagnostic> {
    fund(Some(0))?;
    Ok(index.has_record_invariant(declaration))
}
// Shared walk of `owned_collection_record::admitted` and `nested::record_shape_admitted`.
// Frames and the active set borrow the original types instead of cloning them.
fn shape<'a>(
    index: &'a DeclarationIndex,
    root: &'a ResolvedType,
    depth0: usize,
    fields0: usize,
    strings_mark: bool,
    expected: bool,
) -> Result<bool, Diagnostic> {
    if !matches!(root, ResolvedType::Nominal { declaration, arguments }
        if arguments.is_empty() && index.declaration(declaration).is_some_and(|d| d.kind == DeclarationKind::Record))
    {
        return Ok(false);
    }
    enum Frame<'f> {
        Enter(&'f ResolvedType, usize),
        Leave(&'f DeclarationId),
    }
    let mut pending: Vec<Frame<'a>> = Vec::new();
    let mut active: BTreeSet<&'a DeclarationId> = BTreeSet::new();
    let (mut fields, mut leaves, mut marked) = (fields0, 0usize, false);
    let mut inline = Some(Frame::Enter(root, depth0));
    while let Some(frame) = inline.take().or_else(|| pending.pop()) {
        match frame {
            Frame::Enter(ty, _) if crate::hir::owned_collection_record::vector(index, ty) => {
                marked = true;
                leaves += 1;
            }
            Frame::Enter(ResolvedType::String | ResolvedType::Bytes, _) => {
                marked |= strings_mark;
                leaves += 1;
            }
            Frame::Enter(ty, _) if crate::vec_ops::resolved_element_is_admitted(ty) => {}
            Frame::Enter(
                ResolvedType::Nominal {
                    declaration,
                    arguments,
                },
                depth,
            ) => {
                if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH
                    || !arguments.is_empty()
                    || index
                        .type_parameters(declaration)
                        .is_none_or(|p| !p.is_empty())
                    || index.declaration(declaration).is_none_or(|d| {
                        d.kind != DeclarationKind::Record
                            || d.identity_origin != IdentityOrigin::Explicit
                    })
                    || invariant_present(index, declaration)?
                    || !insert_set(&mut active, declaration)?
                {
                    return Ok(false);
                }
                let Some(declared) = index.record_fields(declaration) else {
                    return Ok(false);
                };
                let Some(next) = fields.checked_add(declared.len()) else {
                    return Ok(false);
                };
                fields = next;
                if fields > crate::cleanup::MAX_CLEANUP_VISITED_FIELDS {
                    return Ok(false);
                }
                for (position, field) in declared.iter().enumerate() {
                    if usize::try_from(field.index) != Ok(position)
                        || index.declaration(&field.id).is_none_or(|d| {
                            d.kind != DeclarationKind::Field
                                || d.identity_origin != IdentityOrigin::Explicit
                                || d.owner.as_ref() != Some(declaration)
                                || d.name != field.name
                        })
                    {
                        return Ok(false);
                    }
                }
                reserve(&mut pending, declared.len() + 1)?;
                pending.push(Frame::Leave(declaration));
                for field in declared.iter().rev() {
                    pending.push(Frame::Enter(&field.ty, depth + 1));
                }
            }
            Frame::Leave(declaration) => {
                active.remove(&declaration);
            }
            Frame::Enter(..) => return Ok(false),
        }
        if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
            return Ok(false);
        }
    }
    Ok(marked == expected)
}
pub(crate) fn record_admitted<'a>(
    root: &'a ResolvedType,
    index: &'a DeclarationIndex,
) -> Result<bool, Diagnostic> {
    fund(Some(0))?;
    shape(index, root, 1, 0, false, true)
}
fn success<'a>(
    index: &'a DeclarationIndex,
    fields: &'a [crate::hir::ResolvedFieldDeclaration],
) -> Result<bool, Diagnostic> {
    match fields {
        [value] => shape(index, &value.ty, 2, 4, true, true),
        _ => Ok(false),
    }
}
fn error(fields: &[crate::hir::ResolvedFieldDeclaration]) -> bool {
    matches!(fields, [code, offset, field]
        if code.ty == ResolvedType::I64 && offset.ty == ResolvedType::Usize && field.ty == ResolvedType::I64)
}
pub(crate) fn nested_admitted<'a>(
    index: &'a DeclarationIndex,
    ty: &'a ResolvedType,
) -> Result<bool, Diagnostic> {
    fund(Some(0))?;
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    if !arguments.is_empty()
        || index
            .type_parameters(declaration)
            .is_none_or(|p| !p.is_empty())
        || index.declaration(declaration).is_none_or(|d| {
            d.kind != DeclarationKind::Variant || d.identity_origin != IdentityOrigin::Explicit
        })
        || invariant_present(index, declaration)?
    {
        return Ok(false);
    }
    let Some(cases) = index.variant_cases(declaration) else {
        return Ok(false);
    };
    if cases.len() != 2 {
        return Ok(false);
    }
    for (position, case) in cases.iter().enumerate() {
        if usize::try_from(case.index) != Ok(position)
            || index.declaration(&case.id).is_none_or(|d| {
                d.kind != DeclarationKind::VariantCase
                    || d.identity_origin != IdentityOrigin::Explicit
                    || d.owner.as_ref() != Some(declaration)
                    || d.name != case.name
            })
        {
            return Ok(false);
        }
        for (position, field) in case.fields.iter().enumerate() {
            if usize::try_from(field.index) != Ok(position)
                || index.declaration(&field.id).is_none_or(|d| {
                    d.kind != DeclarationKind::CaseField
                        || d.identity_origin != IdentityOrigin::Explicit
                        || d.owner.as_ref() != Some(&case.id)
                        || d.name != field.name
                })
            {
                return Ok(false);
            }
        }
    }
    if success(index, &cases[0].fields)? && error(&cases[1].fields) {
        return Ok(true);
    }
    Ok(success(index, &cases[1].fields)? && error(&cases[0].fields))
}
pub(crate) fn runtime_admitted<'a>(
    index: &'a DeclarationIndex,
    ty: &'a ResolvedType,
) -> Result<bool, Diagnostic> {
    fund(Some(0))?;
    Ok(crate::hir::collection_outcome::admitted(index, ty)
        || crate::hir::collection_outcome::owned_admitted(index, ty)
        || nested_admitted(index, ty)?)
}
