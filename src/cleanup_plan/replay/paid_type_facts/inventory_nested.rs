//! Borrowed complete nested receiver. The original independently compiled
//! receiver stays unchanged; physical and work failures are typed errors.
use super::inventory_metadata as metadata;
use super::*;
use crate::hir::{DeclarationKind, IdentityOrigin, ResolvedFieldDeclaration};

enum Frame<'a> {
    Enter(&'a ResolvedType, usize),
    Leave(&'a DeclarationId),
}

pub(super) fn record_shape(
    index: &DeclarationIndex,
    root: &ResolvedType,
    expected_owning: bool,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = root
    else {
        return Ok(false);
    };
    if !arguments.is_empty()
        || metadata::declared(index, declaration, payer)?
            .is_none_or(|d| d.kind != DeclarationKind::Record)
    {
        return Ok(false);
    }
    let mut pending = inventory_flags::vector(payer)?;
    inventory_flags::push(&mut pending, Frame::Enter(root, 2), payer)?;
    let mut active: Vec<&DeclarationId> = inventory_flags::vector(payer)?;
    let mut fields = 4usize;
    let mut leaves = 0usize;
    let mut owned = false;
    loop {
        metadata::step(payer, 1)?;
        let Some(frame) = pending.pop() else {
            break;
        };
        match frame {
            Frame::Enter(ty, depth) => {
                if metadata::vector(index, ty, payer)? {
                    owned = true;
                    leaves += 1;
                } else if matches!(ty, ResolvedType::String | ResolvedType::Bytes) {
                    owned = true;
                    leaves += 1;
                } else if crate::vec_ops::resolved_element_is_admitted(ty) {
                } else if let ResolvedType::Nominal {
                    declaration,
                    arguments,
                } = ty
                {
                    if depth > crate::cleanup::MAX_CLEANUP_SHAPE_DEPTH
                        || !arguments.is_empty()
                        || metadata::parameters(index, declaration, payer)?
                            .is_none_or(|p| !p.is_empty())
                        || metadata::declared(index, declaration, payer)?.is_none_or(|d| {
                            d.kind != DeclarationKind::Record
                                || d.identity_origin != IdentityOrigin::Explicit
                        })
                        || metadata::has_invariant(index, declaration, payer)?
                    {
                        return Ok(false);
                    }
                    for current in &active {
                        if inventory_text::owner_matches(current, declaration, payer)? {
                            return Ok(false);
                        }
                    }
                    inventory_flags::push(&mut active, declaration, payer)?;
                    let Some(declared) = metadata::fields(index, declaration, payer)? else {
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
                        if !field_matches(
                            index,
                            field,
                            position,
                            DeclarationKind::Field,
                            declaration,
                            payer,
                        )? {
                            return Ok(false);
                        }
                    }
                    let additional = declared.len().checked_add(1).ok_or_else(|| {
                        payer.construction_refusal("inventory nested pending length overflowed")
                    })?;
                    metadata::step(payer, additional)?;
                    inventory_flags::grow(&mut pending, additional, payer)?;
                    metadata::step(payer, additional)?; // future scheduled frame releases
                    pending.push(Frame::Leave(declaration));
                    pending.extend(
                        declared
                            .iter()
                            .rev()
                            .map(|field| Frame::Enter(&field.ty, depth + 1)),
                    );
                } else {
                    return Ok(false);
                }
            }
            Frame::Leave(declaration) => {
                // Leave frames are strictly nested, exactly the old active-set
                // removal. No membership authority comes from pointer identity.
                let Some(current) = active.pop() else {
                    return Ok(false);
                };
                if !inventory_text::owner_matches(current, declaration, payer)? {
                    return Ok(false);
                }
            }
        }
        if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
            return Ok(false);
        }
    }
    Ok(owned == expected_owning)
}
fn field_matches(
    index: &DeclarationIndex,
    field: &ResolvedFieldDeclaration,
    position: usize,
    kind: DeclarationKind,
    owner: &DeclarationId,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    if usize::try_from(field.index) != Ok(position) {
        return Ok(false);
    }
    let Some(declared) = metadata::declared(index, &field.id, payer)? else {
        return Ok(false);
    };
    if declared.kind != kind || declared.identity_origin != IdentityOrigin::Explicit {
        return Ok(false);
    }
    let Some(actual_owner) = declared.owner.as_ref() else {
        return Ok(false);
    };
    if !inventory_text::owner_matches(actual_owner, owner, payer)? {
        return Ok(false);
    }
    inventory_text::same_bytes(&declared.name, &field.name, payer)
}
fn success(
    index: &DeclarationIndex,
    fields: &[ResolvedFieldDeclaration],
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let [value] = fields else {
        return Ok(false);
    };
    record_shape(index, &value.ty, true, payer)
}
fn error(fields: &[ResolvedFieldDeclaration], payer: &mut Payer<'_>) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    Ok(
        matches!(fields, [code,offset,field] if code.ty == ResolvedType::I64 && offset.ty == ResolvedType::Usize && field.ty == ResolvedType::I64),
    )
}
pub(super) fn admitted(
    index: &DeclarationIndex,
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    metadata::step(payer, 1)?;
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    if !arguments.is_empty()
        || metadata::parameters(index, declaration, payer)?.is_none_or(|p| !p.is_empty())
        || metadata::declared(index, declaration, payer)?.is_none_or(|d| {
            d.kind != DeclarationKind::Variant || d.identity_origin != IdentityOrigin::Explicit
        })
        || metadata::has_invariant(index, declaration, payer)?
    {
        return Ok(false);
    }
    let Some(cases) = metadata::cases(index, declaration, payer)? else {
        return Ok(false);
    };
    if cases.len() != 2 {
        return Ok(false);
    }
    for (position, case) in cases.iter().enumerate() {
        metadata::step(payer, 1)?;
        if usize::try_from(case.index) != Ok(position) {
            return Ok(false);
        }
        let Some(declared) = metadata::declared(index, &case.id, payer)? else {
            return Ok(false);
        };
        if declared.kind != DeclarationKind::VariantCase
            || declared.identity_origin != IdentityOrigin::Explicit
        {
            return Ok(false);
        }
        let Some(owner) = declared.owner.as_ref() else {
            return Ok(false);
        };
        if !inventory_text::owner_matches(owner, declaration, payer)?
            || !inventory_text::same_bytes(&declared.name, &case.name, payer)?
        {
            return Ok(false);
        }
        for (position, field) in case.fields.iter().enumerate() {
            if !field_matches(
                index,
                field,
                position,
                DeclarationKind::CaseField,
                &case.id,
                payer,
            )? {
                return Ok(false);
            }
        }
    }
    // Preserve both original short-circuit alternatives, including the order
    // in which a semantic false precedes a physical/work refusal.
    if success(index, &cases[0].fields, payer)? && error(&cases[1].fields, payer)? {
        return Ok(true);
    }
    Ok(success(index, &cases[1].fields, payer)? && error(&cases[0].fields, payer)?)
}
pub(super) fn record_field(
    index: &DeclarationIndex,
    container: &ResolvedType,
    case: &DeclarationId,
    field: &DeclarationId,
    ty: &ResolvedType,
    payer: &mut Payer<'_>,
) -> Result<bool, Diagnostic> {
    if !admitted(index, container, payer)? || !record_shape(index, ty, true, payer)? {
        return Ok(false);
    }
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = container
    else {
        return Ok(false);
    };
    if !arguments.is_empty() {
        return Ok(false);
    }
    let Some(cases) = metadata::cases(index, declaration, payer)? else {
        return Ok(false);
    };
    for candidate in cases {
        if !inventory_text::owner_matches(&candidate.id, case, payer)? {
            continue;
        }
        for candidate in &candidate.fields {
            if !inventory_text::owner_matches(&candidate.id, field, payer)? {
                continue;
            }
            metadata::step(payer, 1)?;
            // The completed record-shape receiver proves that ty is Nominal
            // with zero arguments. Derived full ResolvedType equality then
            // compares this discriminant, declaration and the entire empty
            // argument slice. A candidate with any argument remains unequal;
            // no generic tree is truncated or interpreted as equivalent.
            let (
                ResolvedType::Nominal {
                    declaration: actual,
                    arguments: actual_args,
                },
                ResolvedType::Nominal {
                    declaration: expected,
                    arguments: expected_args,
                },
            ) = (&candidate.ty, ty)
            else {
                continue;
            };
            if inventory_text::owner_matches(actual, expected, payer)?
                && actual_args.is_empty()
                && expected_args.is_empty()
            {
                return Ok(true);
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests;
