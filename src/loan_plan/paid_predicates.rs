//! Borrowed predicates for actual caller-paid planner discovery.
//! The legacy allocation-bearing predicates remain independent oracles.
use super::*;
use crate::bounded_output::cfg_reservation::CfgReservation;
use crate::hir::{DeclarationId, DeclarationIndex};

pub(super) fn string_slot(expression: &ResolvedExpr) -> Option<&Place> {
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &expression.kind
    else {
        return None;
    };
    let bytes = match callee.as_str() {
        crate::vec_ops::string_slot::ID => false,
        crate::vec_ops::string_slot::BYTES_ID => true,
        _ => return None,
    };
    let [source, index] = args.as_slice() else {
        return None;
    };
    if type_arguments.as_slice() != [ResolvedType::String]
        || !crate::vec_ops::resolved_vec_matches(&source.ty, &ResolvedType::String)
        || !matches!(source.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
        || index.ty != ResolvedType::Usize
        || index.ownership != OwnershipMode::Value
        || expression.ownership != OwnershipMode::Borrow
        || expression.ty
            != if bytes {
                ResolvedType::SliceU8
            } else {
                ResolvedType::Str
            }
    {
        return None;
    }
    match &source.kind {
        ResolvedExprKind::Place(place) if place.projections.is_empty() => Some(place),
        _ => None,
    }
}

fn admitted(
    root: &ResolvedType,
    index: &DeclarationIndex,
    payer: &CfgReservation,
) -> Result<bool, Diagnostic> {
    if !matches!(root, ResolvedType::Nominal { declaration, arguments }
        if arguments.is_empty() && index.declaration(declaration).is_some_and(|d| d.kind == crate::hir::DeclarationKind::Record))
    {
        return Ok(false);
    }
    enum Frame<'a> {
        Enter(&'a ResolvedType, usize),
        Leave(&'a DeclarationId),
    }
    let mut pending = Vec::new();
    storage::push(&mut pending, Frame::Enter(root, 1), Some(payer))?;
    let mut active = BTreeSet::new();
    let mut fields = 0usize;
    let mut leaves = 0usize;
    let mut has_vector = false;
    while let Some(frame) = pending.pop() {
        match frame {
            Frame::Enter(ty, _) if crate::hir::owned_collection_record::vector(index, ty) => {
                has_vector = true;
                leaves += 1;
            }
            Frame::Enter(ResolvedType::String | ResolvedType::Bytes, _) => leaves += 1,
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
                        d.kind != crate::hir::DeclarationKind::Record
                            || d.identity_origin != crate::hir::IdentityOrigin::Explicit
                    })
                    || invariant(index, declaration, payer)?
                    || !storage::set_insert(&mut active, declaration, Some(payer))?
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
                            d.kind != crate::hir::DeclarationKind::Field
                                || d.identity_origin != crate::hir::IdentityOrigin::Explicit
                                || d.owner.as_ref() != Some(declaration)
                                || d.name != field.name
                        })
                    {
                        return Ok(false);
                    }
                }
                storage::push(&mut pending, Frame::Leave(declaration), Some(payer))?;
                for field in declared.iter().rev() {
                    storage::push(
                        &mut pending,
                        Frame::Enter(&field.ty, depth + 1),
                        Some(payer),
                    )?;
                }
            }
            Frame::Leave(declaration) => {
                active.remove(declaration);
            }
            Frame::Enter(_, _) => return Ok(false),
        }
        if leaves > crate::cleanup::MAX_CLEANUP_OWNED_LEAVES {
            return Ok(false);
        }
    }
    Ok(has_vector)
}

pub(super) fn borrowed_vec_argument(
    program: &dyn crate::cleanup::ReplayContext,
    call: &ResolvedExpr,
    position: usize,
    argument: &ResolvedExpr,
    payer: &CfgReservation,
) -> Result<bool, Diagnostic> {
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        ..
    } = &call.kind
    else {
        return Ok(false);
    };
    let (Some(op), [element]) = (
        crate::vec_ops::by_id(callee.as_str()),
        type_arguments.as_slice(),
    ) else {
        return Ok(false);
    };
    if op.param_ownership_for(position, element) != OwnershipMode::Borrow
        || !crate::hir::owned_collection_record::vector(program.declarations(), &argument.ty)
    {
        return Ok(false);
    }
    let ResolvedExprKind::Place(place) = &argument.kind else {
        return Ok(false);
    };
    let Some(PlaceProjection::Field(first)) = place.projections.first() else {
        return Ok(false);
    };
    let index = program.declarations();
    let Some(root) = index
        .declaration(first)
        .and_then(|field| field.owner.as_ref())
    else {
        return Ok(false);
    };
    let mut ty = ResolvedType::Nominal {
        declaration: root.clone(),
        arguments: Vec::new(),
    };
    if !admitted(&ty, index, payer)? {
        return Ok(false);
    }
    for projection in &place.projections {
        let PlaceProjection::Field(id) = projection else {
            return Ok(false);
        };
        let ResolvedType::Nominal {
            declaration,
            arguments,
        } = &ty
        else {
            return Ok(false);
        };
        let Some(field) = index
            .record_fields(declaration)
            .and_then(|fields| fields.iter().find(|f| f.id == *id))
        else {
            return Ok(false);
        };
        let Some(next) =
            super::paid_substitute::substitute(&field.ty, declaration, arguments, payer)?
        else {
            return Ok(false);
        };
        ty = next;
    }
    Ok(ty == argument.ty)
}

fn invariant(
    index: &DeclarationIndex,
    declaration: &DeclarationId,
    payer: &CfgReservation,
) -> Result<bool, Diagnostic> {
    let Some(length) = declaration.as_str().len().checked_add("#invariant".len()) else {
        payer.arithmetic();
        return Err(storage::marker());
    };
    let mut bytes = storage::vector(length, Some(payer))?;
    bytes.extend_from_slice(declaration.as_str().as_bytes());
    bytes.extend_from_slice(b"#invariant");
    // Both pieces are UTF-8 strings, and conversion moves the prepaid backing.
    let text = String::from_utf8(bytes).expect("identity suffix is UTF-8");
    let id = DeclarationId::paid_owned(text, |bytes| payer.reserve(bytes))
        .map_err(|_| storage::marker())?;
    Ok(index.declaration(&id).is_some())
}
