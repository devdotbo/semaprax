//! Borrowed monomorphic iterator grammar with callbacks at actual operands.
use super::{compare, Admission, Refusal};
use crate::hir::{
    OwnershipMode as M, ResolvedExpr as E, ResolvedExprKind as K, ResolvedMatchArm,
    ResolvedMatchMode, ResolvedMatchPattern as P, ResolvedStatement as S, ResolvedType as T,
    ValueId,
};
fn zero(value: &E, admission: &mut Admission<'_>) -> Result<bool, Refusal> {
    admission.units(1)?;
    Ok(value.ownership == M::Value && matches!(value.kind, K::Int(0) | K::Usize(0)))
}
fn place(value: &E, id: &ValueId, admission: &mut Admission<'_>) -> Result<bool, Refusal> {
    admission.units(1)?;
    let K::Place(place) = &value.kind else {
        return Ok(false);
    };
    Ok(compare::values(&place.root, id, admission)? && place.projections.is_empty())
}
fn cases(arms: &[ResolvedMatchArm], admission: &mut Admission<'_>) -> Result<bool, Refusal> {
    admission.units(1)?;
    let [done, yielded] = arms else {
        return Ok(false);
    };
    if done.guard.is_some() || yielded.guard.is_some() {
        return Ok(false);
    }
    let P::Variant {
        variant,
        case,
        fields,
    } = &done.pattern
    else {
        return Ok(false);
    };
    if !compare::text(variant.as_str(), crate::iterator_ops::STEP_ID, admission)?
        || !compare::text(case.as_str(), crate::iterator_ops::DONE_ID, admission)?
        || !fields.is_empty()
    {
        return Ok(false);
    }
    let P::Variant {
        variant,
        case,
        fields,
    } = &yielded.pattern
    else {
        return Ok(false);
    };
    Ok(
        compare::text(variant.as_str(), crate::iterator_ops::STEP_ID, admission)?
            && compare::text(case.as_str(), crate::iterator_ops::YIELD_ID, admission)?
            && fields.len() == 2
            && compare::text(
                fields[0].field.as_str(),
                crate::iterator_ops::ITEM_ID,
                admission,
            )?
            && compare::text(
                fields[1].field.as_str(),
                crate::iterator_ops::REST_ID,
                admission,
            )?,
    )
}
fn element<'a>(ty: &'a T, admission: &mut Admission<'_>) -> Result<Option<&'a T>, Refusal> {
    admission.units(1)?;
    let T::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(None);
    };
    let [element] = arguments.as_slice() else {
        return Ok(None);
    };
    Ok((compare::text(
        declaration.as_str(),
        crate::iterator_ops::STEP_ID,
        admission,
    )? && (crate::iterator_ops::resolved_element_is_admitted(element)
        || *element == T::String
        || matches!(element, T::Nominal { arguments, .. } if arguments.is_empty())))
    .then_some(element))
}
fn iter(ty: &T, element: &T, admission: &mut Admission<'_>) -> Result<bool, Refusal> {
    admission.units(1)?;
    let T::Nominal {
        declaration,
        arguments,
    } = ty
    else {
        return Ok(false);
    };
    let [argument] = arguments.as_slice() else {
        return Ok(false);
    };
    Ok(compare::text(
        declaration.as_str(),
        crate::iterator_ops::ITER_ID,
        admission,
    )? && compare::types(argument, element, admission)?)
}
fn replacement<'a>(
    value: &'a E,
    binding: &ValueId,
    admission: &mut Admission<'_>,
) -> Result<Option<&'a E>, Refusal> {
    admission.units(1)?;
    let K::Match {
        mode: ResolvedMatchMode::Own,
        scrutinee,
        arms,
    } = &value.kind
    else {
        return Ok(None);
    };
    let Some(element) = element(&value.ty, admission)? else {
        return Ok(None);
    };
    if value.ownership != M::Own
        || !compare::types(&scrutinee.ty, &value.ty, admission)?
        || !place(scrutinee, binding, admission)?
        || !cases(arms, admission)?
    {
        return Ok(None);
    }
    let K::ConstructVariant {
        variant,
        case,
        fields,
    } = &arms[0].value.kind
    else {
        return Ok(None);
    };
    if !compare::text(variant.as_str(), crate::iterator_ops::STEP_ID, admission)?
        || !compare::text(case.as_str(), crate::iterator_ops::DONE_ID, admission)?
        || !fields.is_empty()
        || !compare::types(&arms[0].value.ty, &value.ty, admission)?
    {
        return Ok(None);
    }
    let P::Variant { fields, .. } = &arms[1].pattern else {
        return Ok(None);
    };
    if !compare::types(&fields[0].binding.ty, element, admission)?
        || fields[0].binding.ownership != crate::iterator_ops::item_ownership(element, false)
        || !iter(&fields[1].binding.ty, element, admission)?
        || fields[1].binding.ownership != M::Own
    {
        return Ok(None);
    }
    let K::Block { statements, tail } = &arms[1].value.kind else {
        return Ok(None);
    };
    let [S::Let {
        binding: discard,
        mutable: false,
        value: authored_body,
        ..
    }] = statements.as_slice()
    else {
        return Ok(None);
    };
    if discard.ownership != M::Value
        || !(discard.ty == T::Never || crate::hir::is_scalar_resolved_type(&discard.ty))
        || !compare::types(&discard.ty, &authored_body.ty, admission)?
        || authored_body.ownership != M::Value
    {
        return Ok(None);
    }
    let K::Call {
        callee,
        type_arguments,
        instance: None,
        args,
    } = &tail.kind
    else {
        return Ok(None);
    };
    let [type_argument] = type_arguments.as_slice() else {
        return Ok(None);
    };
    let [argument] = args.as_slice() else {
        return Ok(None);
    };
    if !compare::text(callee.as_str(), crate::iterator_ops::NEXT_ID, admission)?
        || !compare::types(type_argument, element, admission)?
        || !place(argument, &fields[1].binding.id, admission)?
        || !compare::types(&argument.ty, &fields[1].binding.ty, admission)?
        || argument.ownership != M::Own
        || !compare::types(&tail.ty, &value.ty, admission)?
        || tail.ownership != M::Own
    {
        return Ok(None);
    }
    Ok(Some(authored_body))
}
pub(super) fn recognize<'a>(
    condition: &'a E,
    body: &'a E,
    admission: &mut Admission<'_>,
) -> Result<Option<crate::hir::iterator_loop::IteratorLoop<'a>>, Refusal> {
    admission.units(1)?;
    let K::Block { statements, tail } = &body.kind else {
        return Ok(None);
    };
    let [S::Assign {
        binding: step,
        field: None,
        value,
        ..
    }] = statements.as_slice()
    else {
        return Ok(None);
    };
    if !zero(tail, admission)?
        || step.ownership != M::Own
        || !compare::types(&step.ty, &value.ty, admission)?
    {
        return Ok(None);
    }
    let Some(authored_body) = replacement(value, &step.id, admission)? else {
        return Ok(None);
    };
    let K::Match {
        mode: ResolvedMatchMode::Borrow,
        scrutinee,
        arms,
    } = &condition.kind
    else {
        return Ok(None);
    };
    if !place(scrutinee, &step.id, admission)?
        || !compare::types(&scrutinee.ty, &step.ty, admission)?
        || !cases(arms, admission)?
        || condition.ty != T::Bool
        || condition.ownership != M::Value
        || !matches!(arms[0].value.kind, K::Bool(false))
        || !matches!(arms[1].value.kind, K::Bool(true))
    {
        return Ok(None);
    }
    let Some(element) = element(&step.ty, admission)? else {
        return Ok(None);
    };
    let P::Variant { fields, .. } = &arms[1].pattern else {
        return Ok(None);
    };
    if !compare::types(&fields[0].binding.ty, element, admission)?
        || fields[0].binding.ownership != crate::iterator_ops::item_ownership(element, true)
        || !iter(&fields[1].binding.ty, element, admission)?
        || fields[1].binding.ownership != M::Borrow
    {
        return Ok(None);
    }
    let owned_item = if crate::iterator_ops::item_ownership(element, false) == M::Own {
        let K::Match { arms, .. } = &value.kind else {
            return Ok(None);
        };
        let P::Variant { fields, .. } = &arms[1].pattern else {
            return Ok(None);
        };
        Some(&fields[0].binding)
    } else {
        None
    };
    Ok(Some(crate::hir::iterator_loop::IteratorLoop {
        step,
        authored_body,
        owned_item,
    }))
}
