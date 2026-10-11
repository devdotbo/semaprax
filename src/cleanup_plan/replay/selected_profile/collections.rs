//! Actual monomorphic Vec and field-read facts, independent of plan metadata.
use super::*;
use crate::vec_ops::VecOp;

pub(in crate::cleanup_plan::replay) fn call(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    let ResolvedExprKind::Call {
        callee,
        instance,
        type_arguments,
        args,
    } = &expression.kind
    else {
        return Ok(false);
    };
    let Some(op) = crate::vec_ops::by_id(callee.as_str()) else {
        return Ok(false);
    };
    if instance.is_some() || op == VecOp::SortBy || op == VecOp::StrAt {
        return Err(replay_error(
            function,
            "selected Vec call requires a separately authenticated operation",
        ));
    }
    let [element] = type_arguments.as_slice() else {
        return Err(replay_error(
            function,
            "selected Vec call has invalid type arguments",
        ));
    };
    let vector = if op == VecOp::WithCapacity {
        &expression.ty
    } else {
        &args
            .first()
            .ok_or_else(|| replay_error(function, "selected Vec source is absent"))?
            .ty
    };
    if crate::cleanup::selected_vector_element(program, vector)? != Some(element) {
        return Err(replay_error(
            function,
            "selected Vec element and root disagree",
        ));
    }
    let owned = *element == ResolvedType::Bytes
        || *element == ResolvedType::String
        || crate::hir::owned_leaf_collection::runtime_element(program.declarations(), element);
    let permitted = crate::vec_ops::resolved_operation_element_is_admitted(op, element)
        || crate::hir::owned_leaf_collection::vec_operation_admitted(
            program.declarations(),
            op,
            element,
        );
    if !permitted || args.len() != op.arity() {
        return Err(replay_error(
            function,
            "selected Vec operation or arity is outside the exact element family",
        ));
    }
    budget.charge(function, args.len() + 9, "selected Vec argument facts")?;
    for (i, arg) in args.iter().enumerate() {
        let expected = if owned {
            op.param_ownership_for(i, element)
        } else {
            op.param_ownership(i)
        };
        let valid_type = match (op, i) {
            (VecOp::WithCapacity, 0) => arg.ty == ResolvedType::Usize,
            (_, 0) => crate::cleanup::selected_vector_element(program, &arg.ty)? == Some(element),
            (VecOp::Push, 1) | (VecOp::Set | VecOp::Replace, 2) => &arg.ty == element,
            _ => arg.ty == ResolvedType::Usize,
        };
        if !valid_type
            || (expected != arg.ownership
                && !(expected == OwnershipMode::Borrow
                    && matches!(arg.ownership, OwnershipMode::Own | OwnershipMode::Borrow)))
        {
            return Err(replay_error(
                function,
                "selected Vec argument type or mode disagrees",
            ));
        }
    }
    let (valid_type, mode) = match op {
        VecOp::Len | VecOp::Capacity => {
            (expression.ty == ResolvedType::Usize, OwnershipMode::Value)
        }
        VecOp::Get | VecOp::CloneAt => (
            &expression.ty == element,
            if owned {
                OwnershipMode::Own
            } else {
                OwnershipMode::Value
            },
        ),
        _ => (
            crate::cleanup::selected_vector_element(program, &expression.ty)? == Some(element),
            OwnershipMode::Own,
        ),
    };
    if !valid_type || expression.ownership != mode {
        return Err(replay_error(
            function,
            "selected Vec result type or mode disagrees",
        ));
    }
    Ok(true)
}

pub(in crate::cleanup_plan::replay) fn field(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    let ResolvedExprKind::VecFieldRead {
        element,
        field,
        bytes,
        args,
    } = &expression.kind
    else {
        return Ok(());
    };
    let [owner, index] = args.as_slice() else {
        return Err(replay_error(
            function,
            "selected Vec field read has invalid arity",
        ));
    };
    let Some((_, fields)) = crate::cleanup::selected_flat_record_fields(program, element)? else {
        return Err(replay_error(
            function,
            "selected Vec field element is not current flat record",
        ));
    };
    budget.charge(function, fields.len() + 9, "selected Vec field facts")?;
    let declared = fields
        .iter()
        .find(|f| &f.id == field)
        .ok_or_else(|| replay_error(function, "selected Vec field identity is foreign"))?;
    let (ty, mode) = match &declared.ty {
        ResolvedType::String => (
            if *bytes {
                &ResolvedType::SliceU8
            } else {
                &ResolvedType::Str
            },
            OwnershipMode::Borrow,
        ),
        ResolvedType::Bytes if !bytes => (&ResolvedType::SliceU8, OwnershipMode::Borrow),
        ty if crate::hir::is_scalar_resolved_type(ty) && !*bytes => (ty, OwnershipMode::Value),
        _ => {
            return Err(replay_error(
                function,
                "selected Vec field mode is unsupported",
            ))
        }
    };
    if crate::cleanup::selected_vector_element(program, &owner.ty)? != Some(element)
        || !matches!(&owner.kind,ResolvedExprKind::Place(p) if p.projections.is_empty())
        || !matches!(owner.ownership, OwnershipMode::Own | OwnershipMode::Borrow)
        || index.ty != ResolvedType::Usize
        || index.ownership != OwnershipMode::Value
        || &expression.ty != ty
        || expression.ownership != mode
    {
        return Err(replay_error(
            function,
            "selected Vec field root, index, result type or mode disagrees",
        ));
    }
    Ok(())
}

pub(in crate::cleanup_plan::replay) fn append(
    function: &ResolvedFunction,
    binding: &crate::hir::ResolvedBinding,
    value: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    let ReplayBudget {
        remaining,
        physical,
        ..
    } = budget;
    let physical = physical
        .as_deref_mut()
        .ok_or_else(|| replay_error(function, "selected append lost reservation"))?;
    crate::cleanup_plan::renewal_profile::selected_string_append(
        binding,
        value,
        physical,
        &mut |units| {
            if let Some(next) = remaining.checked_sub(units) {
                *remaining = next;
                Ok(())
            } else {
                *remaining = 0;
                Err(replay_error(function, "selected append work exhausted"))
            }
        },
    )
}
