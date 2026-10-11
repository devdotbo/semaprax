//! Current borrowed-record scalar reads. This receiver is not defining-source
//! authority: the held producer, ordinary HIR and complete replay stay required.
use super::*;

pub(super) fn signature(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    if !crate::hir::is_scalar_resolved_type(&function.return_type) {
        return Ok(false);
    }
    for parameter in &function.params {
        if crate::hir::borrowed_scalar::parameter(&parameter.ty, parameter.ownership) {
            continue;
        }
        if parameter.ownership != OwnershipMode::Borrow
            || !matches!(&parameter.ty, ResolvedType::Nominal { arguments, .. } if arguments.is_empty())
        {
            return Ok(false);
        }
        fields(program, function, &parameter.ty, budget)?;
    }
    Ok(true)
}

fn fields<'a>(
    program: &'a dyn ReplayContext,
    function: &ResolvedFunction,
    ty: &'a ResolvedType,
    budget: &mut ReplayBudget,
) -> Result<&'a [crate::hir::ResolvedFieldDeclaration], Diagnostic> {
    selected_profile::collections::prepay(
        program,
        function,
        selected_profile::collections::type_key_bytes(ty)?,
        budget,
    )?;
    crate::cleanup::selected_flat_record_fields(program, ty)?
        .map(|(_, fields)| fields)
        .ok_or_else(|| replay_error(function, "scalar record helper lacks a current flat layout"))
}

fn root(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    let ResolvedExprKind::Place(place) = &expression.kind else {
        return Ok(false);
    };
    if expression.ownership != OwnershipMode::Borrow
        || !place.projections.is_empty()
        || !matches!(&expression.ty, ResolvedType::Nominal { arguments, .. } if arguments.is_empty())
    {
        return Ok(false);
    }
    fields(program, function, &expression.ty, budget)?;
    let mut found = false;
    for parameter in &function.params {
        budget.charge(
            function,
            parameter
                .id
                .as_str()
                .len()
                .checked_add(place.root.as_str().len())
                .and_then(|n| n.checked_add(1))
                .ok_or_else(|| replay_error(function, "scalar record root work overflow"))?,
            "scalar borrowed record parameter identity",
        )?;
        if parameter.id == place.root {
            budget.charge(
                function,
                selected_profile::collections::type_key_bytes(&parameter.ty)?
                    .checked_add(selected_profile::collections::type_key_bytes(
                        &expression.ty,
                    )?)
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(|| replay_error(function, "scalar record type work overflow"))?,
                "scalar borrowed record parameter type identity",
            )?;
            if found
                || parameter.ownership != OwnershipMode::Borrow
                || parameter.ty != expression.ty
            {
                return Ok(false);
            }
            found = true;
        }
    }
    Ok(found)
}

pub(super) fn expression(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<Option<bool>, Diagnostic> {
    match &expression.kind {
        ResolvedExprKind::Place(_) if matches!(expression.ty, ResolvedType::Nominal { .. }) => {
            Ok(Some(root(program, function, expression, budget)?))
        }
        ResolvedExprKind::Project { base, field }
            if matches!(base.ty, ResolvedType::Nominal { .. }) =>
        {
            if !root(program, function, base, budget)?
                || expression.ownership != OwnershipMode::Value
                || !crate::hir::is_scalar_resolved_type(&expression.ty)
            {
                return Ok(Some(false));
            }
            let declared = fields(program, function, &base.ty, budget)?;
            budget.charge(
                function,
                field
                    .as_str()
                    .len()
                    .checked_add(1)
                    .and_then(|n| n.checked_mul(declared.len()))
                    .ok_or_else(|| replay_error(function, "scalar record field work overflow"))?,
                "scalar record field identity",
            )?;
            let mut found = None;
            for current in declared {
                if current.id == *field && found.replace(current).is_some() {
                    return Ok(Some(false));
                }
            }
            Ok(Some(found.is_some_and(|current| {
                crate::hir::is_scalar_resolved_type(&current.ty) && current.ty == expression.ty
            })))
        }
        _ => Ok(None),
    }
}

#[cfg(test)]
pub(in crate::cleanup_plan::replay) fn expression_for_test(
    program: &dyn ReplayContext,
    function: &ResolvedFunction,
    expression: &ResolvedExpr,
    budget: &mut ReplayBudget,
) -> Result<Option<bool>, Diagnostic> {
    self::expression(program, function, expression, budget)
}
