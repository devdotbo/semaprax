//! Exact comparison receiver for RowLoan's independent structural prefix.
//! Legacy traversal callers keep their original classifier. No authored body
//! walk, cached hint or additional prefix-index allocation participates.
use super::*;

pub(crate) enum Comparison<'a> {
    Text(&'a str, &'a str),
    Type(&'a ResolvedType, &'a ResolvedType),
    Value(&'a ValueId, &'a ValueId),
    Binding(&'a ResolvedBinding, &'a ResolvedBinding),
    Element(&'a ResolvedType),
    Visit,
}
enum Failure<E> {
    Invalid,
    Work(E),
}
fn paid<E>(
    charge: &mut impl FnMut(Comparison<'_>) -> Result<(), E>,
    comparison: Comparison<'_>,
) -> Result<(), Failure<E>> {
    charge(comparison).map_err(Failure::Work)
}
fn text_equal<E>(
    left: &str,
    right: &str,
    charge: &mut impl FnMut(Comparison<'_>) -> Result<(), E>,
) -> Result<bool, Failure<E>> {
    paid(charge, Comparison::Text(left, right))?;
    Ok(left == right)
}
fn type_equal<E>(
    left: &ResolvedType,
    right: &ResolvedType,
    charge: &mut impl FnMut(Comparison<'_>) -> Result<(), E>,
) -> Result<bool, Failure<E>> {
    paid(charge, Comparison::Type(left, right))?;
    Ok(left == right)
}
fn value_equal<E>(
    left: &ValueId,
    right: &ValueId,
    charge: &mut impl FnMut(Comparison<'_>) -> Result<(), E>,
) -> Result<bool, Failure<E>> {
    paid(charge, Comparison::Value(left, right))?;
    Ok(left == right)
}
fn binding_equal<E>(
    left: &ResolvedBinding,
    right: &ResolvedBinding,
    charge: &mut impl FnMut(Comparison<'_>) -> Result<(), E>,
) -> Result<bool, Failure<E>> {
    paid(charge, Comparison::Binding(left, right))?;
    Ok(left == right)
}
fn scalar<E>(
    binding: &ResolvedBinding,
    charge: &mut impl FnMut(Comparison<'_>) -> Result<(), E>,
) -> Result<bool, Failure<E>> {
    paid(charge, Comparison::Visit)?;
    Ok(type_equal(&binding.ty, &ResolvedType::Usize, charge)?
        && binding.ownership == OwnershipMode::Value)
}
fn usize_value<E>(
    expression: &ResolvedExpr,
    expected: usize,
    charge: &mut impl FnMut(Comparison<'_>) -> Result<(), E>,
) -> Result<bool, Failure<E>> {
    paid(charge, Comparison::Visit)?;
    Ok(type_equal(&expression.ty, &ResolvedType::Usize, charge)?
        && expression.ownership == OwnershipMode::Value
        && matches!(expression.kind, ResolvedExprKind::Usize(value) if value == expected as u64))
}
fn place_is<E>(
    expression: &ResolvedExpr,
    binding: &ResolvedBinding,
    charge: &mut impl FnMut(Comparison<'_>) -> Result<(), E>,
) -> Result<bool, Failure<E>> {
    paid(charge, Comparison::Visit)?;
    if !type_equal(&expression.ty, &binding.ty, charge)?
        || expression.ownership != binding.ownership
    {
        return Ok(false);
    }
    let ResolvedExprKind::Place(place) = &expression.kind else {
        return Ok(false);
    };
    Ok(value_equal(&place.root, &binding.id, charge)? && place.projections.is_empty())
}
fn source_is_vec<E>(
    ty: &ResolvedType,
    element: &ResolvedType,
    charge: &mut impl FnMut(Comparison<'_>) -> Result<(), E>,
) -> Result<bool, Failure<E>> {
    paid(charge, Comparison::Visit)?;
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    Ok(
        text_equal(declaration.as_str(), crate::prelude::VEC_ID, charge)?
            && arguments.len() == 1
            && type_equal(&arguments[0], element, charge)?,
    )
}
fn admitted<E>(
    element: &ResolvedType,
    predicate: &impl Fn(&ResolvedType) -> bool,
    charge: &mut impl FnMut(Comparison<'_>) -> Result<(), E>,
) -> Result<bool, Failure<E>> {
    paid(charge, Comparison::Element(element))?;
    Ok(predicate(element))
}
pub(crate) fn row_region<'a, E>(
    index: &crate::hir::DeclarationIndex,
    region: &'a ResolvedExpr,
    charge: &mut impl FnMut(Comparison<'_>) -> Result<(), E>,
) -> Result<Option<RowRegion<'a>>, E> {
    match prefix(
        region,
        |element| crate::hir::row_loan::element_admitted(index, element),
        charge,
    ) {
        Ok(Some(prefix)) => Ok(Some(RowRegion {
            source: prefix.source,
            owner: prefix.owner,
            element: prefix.element,
            length: &prefix.length.id,
            row_index: &prefix.row_index.id,
        })),
        Ok(None) | Err(Failure::Invalid) => Ok(None),
        Err(Failure::Work(error)) => Err(error),
    }
}
fn prefix<'a, E>(
    region: &'a ResolvedExpr,
    element_admitted: impl Fn(&ResolvedType) -> bool,
    charge: &mut impl FnMut(Comparison<'_>) -> Result<(), E>,
) -> Result<Option<Prefix<'a>>, Failure<E>> {
    paid(charge, Comparison::Visit)?;
    let ResolvedExprKind::Block { statements, tail } = &region.kind else {
        return Ok(None);
    };
    paid(charge, Comparison::Visit)?;
    let Some(ResolvedStatement::Let {
        binding: marker, ..
    }) = statements.first()
    else {
        return Ok(None);
    };
    if !text_equal(&marker.name, "#for-borrow-length", charge)? {
        return Ok(None);
    }
    paid(charge, Comparison::Visit)?;
    let [ResolvedStatement::Let {
        binding: length,
        mutable: false,
        value: length_value,
        ..
    }, ResolvedStatement::Let {
        binding: counter,
        mutable: true,
        value: zero,
        ..
    }, ResolvedStatement::While {
        condition, body, ..
    }
    | ResolvedStatement::LexicalWhile {
        condition, body, ..
    }] = statements.as_slice()
    else {
        return Err(Failure::Invalid);
    };
    if !scalar(length, charge)?
        || !scalar(counter, charge)?
        || !usize_value(zero, 0, charge)?
        || !usize_value(tail, 0, charge)?
        || !type_equal(&region.ty, &ResolvedType::Usize, charge)?
        || region.ownership != OwnershipMode::Value
    {
        return Err(Failure::Invalid);
    }
    paid(charge, Comparison::Visit)?;
    let ResolvedExprKind::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &length_value.kind
    else {
        return Err(Failure::Invalid);
    };
    paid(charge, Comparison::Visit)?;
    let [element] = type_arguments.as_slice() else {
        return Err(Failure::Invalid);
    };
    paid(charge, Comparison::Visit)?;
    let [source] = args.as_slice() else {
        return Err(Failure::Invalid);
    };
    paid(charge, Comparison::Visit)?;
    let ResolvedExprKind::Place(owner) = &source.kind else {
        return Err(Failure::Invalid);
    };
    if !text_equal(callee.as_str(), crate::vec_ops::LEN_ID, charge)?
        || !owner.projections.is_empty()
        || !source_is_vec(&source.ty, element, charge)?
        || !matches!(source.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
        || !admitted(element, &element_admitted, charge)?
        || !type_equal(&length_value.ty, &ResolvedType::Usize, charge)?
        || length_value.ownership != OwnershipMode::Value
    {
        return Err(Failure::Invalid);
    }
    paid(charge, Comparison::Visit)?;
    let ResolvedExprKind::Binary {
        op: BinaryOp::Lt,
        left,
        right,
    } = &condition.kind
    else {
        return Err(Failure::Invalid);
    };
    if !place_is(left, counter, charge)?
        || !place_is(right, length, charge)?
        || !type_equal(&condition.ty, &ResolvedType::Bool, charge)?
    {
        return Err(Failure::Invalid);
    }
    paid(charge, Comparison::Visit)?;
    let ResolvedExprKind::Block {
        statements: iteration,
        tail: iteration_tail,
    } = &body.kind
    else {
        return Err(Failure::Invalid);
    };
    paid(charge, Comparison::Visit)?;
    let [ResolvedStatement::Let {
        binding: row_index,
        mutable: false,
        value: index_snapshot,
        ..
    }, ResolvedStatement::Let {
        binding: discarded,
        mutable: false,
        value: authored,
        ..
    }, ResolvedStatement::Assign {
        binding: incremented,
        field: None,
        value: increment,
        ..
    }] = iteration.as_slice()
    else {
        return Err(Failure::Invalid);
    };
    if !scalar(row_index, charge)?
        || !place_is(index_snapshot, counter, charge)?
        || !binding_equal(incremented, counter, charge)?
        || !type_equal(&discarded.ty, &authored.ty, charge)?
        || discarded.ownership != authored.ownership
        || !usize_value(iteration_tail, 0, charge)?
    {
        return Err(Failure::Invalid);
    }
    let ids = [
        &owner.root,
        &length.id,
        &counter.id,
        &row_index.id,
        &discarded.id,
    ];
    for (index, id) in ids.iter().enumerate() {
        for previous in &ids[..index] {
            if value_equal(id, previous, charge)? {
                return Err(Failure::Invalid);
            }
        }
    }
    paid(charge, Comparison::Visit)?;
    let ResolvedExprKind::Binary {
        op: BinaryOp::Add,
        left,
        right,
    } = &increment.kind
    else {
        return Err(Failure::Invalid);
    };
    if !place_is(left, counter, charge)? || !usize_value(right, 1, charge)? {
        return Err(Failure::Invalid);
    }
    Ok(Some(Prefix {
        source,
        owner,
        element,
        counter,
        length,
        row_index,
        authored,
    }))
}
#[cfg(test)]
mod tests;
