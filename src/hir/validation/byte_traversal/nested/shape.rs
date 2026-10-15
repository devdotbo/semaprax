//! Local receiver structure only; never a complete provenance/owner verdict.
//! Contextual census must independently freeze every enclosing region.
use super::super::*;

#[derive(Clone, Copy)]
pub(super) struct Shape<'a> {
    pub(super) root: &'a ResolvedExpr,
    pub(super) receiver: &'a ResolvedExpr,
    pub(super) authored_body: &'a ResolvedExpr,
    pub(super) view: &'a ResolvedExpr,
    pub(super) payload: &'a ResolvedBinding,
    pub(super) frozen: [&'a ValueId; 6],
}
fn fail(message: &'static str) -> Diagnostic {
    Diagnostic::io_static("SPX-H006", message)
}
pub(super) fn shape(region: &ResolvedExpr) -> Result<Option<Shape<'_>>, Diagnostic> {
    let ResolvedExprKind::Block { statements, tail } = &region.kind else {
        return Ok(None);
    };
    let [ResolvedStatement::Let {
        binding: length,
        mutable: false,
        value: count,
        ..
    }, ResolvedStatement::Let {
        binding: counter,
        mutable: true,
        value: seed,
        ..
    }, looped] = statements.as_slice()
    else {
        return Ok(None);
    };
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &count.kind
    else {
        return Ok(None);
    };
    if callee.as_str() != crate::byte_ops::LEN_ID {
        return Ok(None);
    }
    let (condition, body, latch) = match looped {
        ResolvedStatement::While {
            condition, body, ..
        } => (condition, body, None),
        ResolvedStatement::LexicalWhile {
            condition,
            body,
            latch,
            ..
        } => (condition, body, Some(latch)),
        _ => return Ok(None),
    };
    let ResolvedExprKind::Block {
        statements: steps,
        tail: step_tail,
    } = &body.kind
    else {
        return Ok(None);
    };
    let [ResolvedStatement::Let {
        binding: item,
        mutable: false,
        value: read,
        ..
    }, ResolvedStatement::Let {
        binding: discard,
        mutable: false,
        value: authored_body,
        ..
    }, ResolvedStatement::Assign {
        binding: assigned,
        field: None,
        value: advance,
        ..
    }] = steps.as_slice()
    else {
        return Ok(None);
    };
    let Some(proof) = receiver_shape(read) else {
        return Ok(None);
    };
    let inconsistent = || fail("borrowed byte traversal differs from its authenticated lowering");
    let [view] = args.as_slice() else {
        return Err(inconsistent());
    };
    let ResolvedExprKind::Binary {
        op: crate::ast::BinaryOp::Lt,
        left,
        right,
    } = &condition.kind
    else {
        return Err(inconsistent());
    };
    let ResolvedExprKind::Binary {
        op: crate::ast::BinaryOp::Add,
        left: old,
        right: one,
    } = &advance.kind
    else {
        return Err(inconsistent());
    };
    let Some(view_place) = bare_place(view) else {
        return Err(inconsistent());
    };
    if !type_arguments.is_empty()
        || view.ty != ResolvedType::SliceU8
        || proof.view.ty != view.ty
        || proof.view.ownership != view.ownership
        || bare_place(proof.view) != Some(view_place)
        || count.ty != ResolvedType::Usize
        || count.ownership != OwnershipMode::Value
        || length.ty != ResolvedType::Usize
        || length.ownership != OwnershipMode::Value
        || counter.ty != ResolvedType::Usize
        || counter.ownership != OwnershipMode::Value
        || !zero(seed)
        || !zero(tail)
        || region.ty != ResolvedType::Usize
        || region.ownership != OwnershipMode::Value
        || condition.ty != ResolvedType::Bool
        || condition.ownership != OwnershipMode::Value
        || !place_of(left, counter)
        || !place_of(right, length)
        || !place_of(proof.index, counter)
        || item.ty != ResolvedType::U8
        || item.ownership != OwnershipMode::Value
        || discard.ty != authored_body.ty
        || discard.ownership != authored_body.ownership
        || assigned.id != counter.id
        || assigned.ty != counter.ty
        || advance.ty != ResolvedType::Usize
        || advance.ownership != OwnershipMode::Value
        || !place_of(old, counter)
        || !matches!(one.kind, ResolvedExprKind::Usize(1))
        || one.ty != ResolvedType::Usize
        || one.ownership != OwnershipMode::Value
        || !zero(step_tail)
    {
        return Err(inconsistent());
    }
    if let Some(latch) = latch {
        if !matches!(latch, LoopLatchKind::SourceForStep { kind: SourceForKind::BorrowBytes, step } if step == &advance.id)
        {
            return Err(inconsistent());
        }
    }
    Ok(Some(Shape {
        root: region,
        frozen: [
            &length.id,
            &counter.id,
            &item.id,
            &discard.id,
            &proof.payload.id,
            &view_place.root,
        ],
        receiver: read,
        authored_body,
        view,
        payload: proof.payload,
    }))
}
