//! A local descriptor never stands in for owned storage. Reconstruct static
//! roots from the exact immutable Str initializer chain in the current HIR.
use super::*;
use crate::hir::{ResolvedBinding, ValueId};

pub(crate) struct StaticOrigin<'a> {
    pub(crate) descriptor: &'a ValueId,
    pub(crate) producer: &'a crate::hir::ExpressionId,
    pub(crate) length: u64,
}

pub(crate) fn str_origin<'a>(
    root: &ValueId,
    bindings: &[(&'a ResolvedBinding, bool, &'a ResolvedExpr)],
) -> Result<Option<StaticOrigin<'a>>, Diagnostic> {
    let mut current = root;
    let mut work = WORK;
    for _ in 0..DEPTH {
        let mut found = None;
        for (binding, mutable, expression) in bindings {
            work = work
                .checked_sub(1)
                .ok_or_else(|| hir_error("static text alias proof exceeds work limit"))?;
            if &binding.id == current {
                if found.is_some() {
                    return Err(hir_error(
                        "static text alias has duplicate binding identity",
                    ));
                }
                found = Some((*binding, *mutable, *expression));
            }
        }
        let Some((binding, mutable, expression)) = found else {
            return Ok(None);
        };
        if mutable
            || binding.ty != ResolvedType::Str
            || binding.ownership != OwnershipMode::Borrow
            || expression.ty != ResolvedType::Str
            || expression.ownership != OwnershipMode::Borrow
        {
            return Err(hir_error(
                "static text alias has invalid storage or ownership",
            ));
        }
        match &expression.kind {
            ResolvedExprKind::BorrowedUtf8Literal(value) => {
                validate_literal(value)?;
                if !value.is_backed() {
                    return Err(hir_error("static text backing allocation was refused"));
                }
                return Ok(Some(StaticOrigin {
                    descriptor: &binding.id,
                    producer: &expression.id,
                    length: value.len() as u64,
                }));
            }
            ResolvedExprKind::Place(place) if place.projections.is_empty() => current = &place.root,
            _ => return Ok(None),
        }
    }
    Err(hir_error(
        "static text alias proof exceeds depth or contains a cycle",
    ))
}
