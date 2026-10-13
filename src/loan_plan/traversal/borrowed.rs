//! Fixed-depth borrowed freeze reconstruction for the finite construction proof.
use super::*;
use crate::hir::lexical_exit::ChildCursor;
const DEPTH: usize = crate::cache_codec::MAX_DEPTH + 1;
#[derive(Clone, Copy)]
struct Frame<'a> {
    cursor: ChildCursor<'a>,
    permitted_root: bool,
}
struct Scratch<'a> {
    frames: [Option<Frame<'a>>; DEPTH],
    len: usize,
}
fn whole_vector(expression: &ResolvedExpr, proof: &Prefix<'_>) -> bool {
    expression.ty == proof.source.ty
        && expression.ownership == proof.source.ownership
        && matches!(&expression.kind,ResolvedExprKind::Place(place) if place==proof.owner)
}
// Permissions attach to this exact parent's child pointer, not a copied ID.
fn permits(
    program: &dyn crate::cleanup::ReplayContext,
    parent: &ResolvedExpr,
    child: &ResolvedExpr,
    proof: &Prefix<'_>,
) -> Option<bool> {
    match &parent.kind {
        ResolvedExprKind::VecFieldRead {
            origin,
            element,
            field,
            bytes,
            args,
        } => {
            let [vector, index] = args.as_slice() else {
                return None;
            };
            if !matches!(&vector.kind,ResolvedExprKind::Place(p) if p.root==proof.owner.root) {
                return Some(false);
            }
            let selected = crate::hir::vec_field::field(program.declarations(), element, field)?;
            // Result is a primitive view or scalar: compare without cloning any type.
            let result_matches = match (&selected.declaration.ty, *bytes) {
                (ResolvedType::String, false) => {
                    parent.ty == ResolvedType::Str && parent.ownership == OwnershipMode::Borrow
                }
                (ResolvedType::String, true) | (ResolvedType::Bytes, false) => {
                    parent.ty == ResolvedType::SliceU8 && parent.ownership == OwnershipMode::Borrow
                }
                (ResolvedType::I64, false) => {
                    parent.ty == ResolvedType::I64 && parent.ownership == OwnershipMode::Value
                }
                _ => false,
            };
            if !whole_vector(vector, proof)
                || element != proof.element
                || !field_index(*origin, index, proof.row_index)
                || !result_matches
            {
                return None;
            }
            Some(std::ptr::eq(child, vector))
        }
        ResolvedExprKind::Call {
            callee,
            instance: None,
            type_arguments,
            args,
        } if matches!(
            callee.as_str(),
            crate::vec_ops::GET_ID | crate::vec_ops::LEN_ID
        ) =>
        {
            let vector = args.first()?;
            if !matches!(&vector.kind,ResolvedExprKind::Place(p) if p.root==proof.owner.root) {
                return Some(false);
            }
            if !whole_vector(vector, proof)
                || type_arguments.as_slice() != std::slice::from_ref(proof.element)
            {
                return None;
            }
            if callee.as_str() == crate::vec_ops::GET_ID {
                let [_, index] = args.as_slice() else {
                    return None;
                };
                if !place_is(index, proof.row_index)
                    || parent.ty != *proof.element
                    || parent.ownership != OwnershipMode::Value
                {
                    return None;
                }
                // In this selected profile GET is only the exact child of an
                // authenticated scalar Project; caller checks that parent edge.
            } else if args.len() != 1
                || parent.ty != ResolvedType::Usize
                || parent.ownership != OwnershipMode::Value
            {
                return None;
            }
            Some(std::ptr::eq(child, vector))
        }
        _ => Some(false),
    }
}
pub(crate) fn inspect<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    region: &'a ResolvedExpr,
    expected_element: &ResolvedType,
    work: &mut usize,
) -> Option<Option<Proof<'a>>> {
    let Some(proof) = prefix(region, |element| element == expected_element).ok()? else {
        return Some(None);
    };
    if matches!(&proof.authored.kind, ResolvedExprKind::Call { callee, .. } if callee.as_str()==crate::vec_ops::GET_ID)
    {
        return None;
    }
    if !crate::bounded_output::reserve_active_required(std::mem::size_of::<Scratch<'_>>()) {
        return None;
    }
    let mut scratch = Scratch {
        frames: [None; DEPTH],
        len: 1,
    };
    scratch.frames[0] = Some(Frame {
        cursor: ChildCursor::new(proof.authored),
        permitted_root: false,
    });
    while scratch.len != 0 {
        *work = work.checked_sub(1)?;
        let frame = scratch.frames[scratch.len - 1].as_mut()?;
        let expression = frame.cursor.expression();
        if !frame.cursor.is_started() {
            permits(program, expression, expression, &proof)?;
            match &expression.kind {
                ResolvedExprKind::Place(place)
                    if place.root == proof.owner.root && !frame.permitted_root =>
                {
                    return None
                }
                ResolvedExprKind::BorrowPlace { place, .. } if place.root == proof.owner.root => {
                    return None
                }
                ResolvedExprKind::Block { statements, .. }
                | ResolvedExprKind::VecConstructor { statements, .. } => {
                    for statement in statements {
                        *work = work.checked_sub(1)?;
                        if matches!(statement,ResolvedStatement::Assign{binding,..} if binding.id==proof.owner.root || binding.id==proof.counter.id || binding.id==proof.row_index.id || binding.id==proof.length.id)
                            || matches!(statement,ResolvedStatement::Let{binding,..} if binding.name=="#for-borrow-length" || binding.id==proof.owner.root || binding.id==proof.counter.id || binding.id==proof.row_index.id || binding.id==proof.length.id)
                        {
                            return None;
                        }
                    }
                }
                _ => {}
            }
        }
        if let Some(child) = frame.cursor.next() {
            let permitted_root = permits(program, expression, child, &proof)?;
            // The non-Copy row getter cannot produce a value/owner anywhere
            // except this immediate exact scalar projection edge.
            if matches!(&child.kind,ResolvedExprKind::Call{callee,..} if callee.as_str()==crate::vec_ops::GET_ID)
            {
                let ResolvedExprKind::Project { base, field } = &expression.kind else {
                    return None;
                };
                let selected =
                    crate::hir::vec_field::field(program.declarations(), proof.element, field)?;
                if !std::ptr::eq(child, base.as_ref())
                    || selected.declaration.ty != ResolvedType::I64
                    || expression.ty != ResolvedType::I64
                    || expression.ownership != OwnershipMode::Value
                {
                    return None;
                }
            }
            *scratch.frames.get_mut(scratch.len)? = Some(Frame {
                cursor: ChildCursor::new(child),
                permitted_root,
            });
            scratch.len += 1;
        } else {
            scratch.frames[scratch.len - 1] = None;
            scratch.len -= 1;
        }
    }
    Some(Some(Proof {
        source: proof.source,
        region,
    }))
}
