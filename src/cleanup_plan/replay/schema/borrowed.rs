//! Guarded complete nonowning helper profile, bound to actual current members.
//! This witness carries no replay verdict and does not select a cleanup schema.
use super::*;
use crate::hir::{OwnershipMode, ResolvedStatement};
const DEPTH: usize = 514;
type Frame<'a> = Option<(crate::hir::lexical_exit::ChildCursor<'a>, bool)>;

pub(super) struct Profile<'a> {
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    inventory: crate::cleanup::CleanupInventory,
}
impl<'a> Profile<'a> {
    pub(super) fn matches(
        &self,
        program: &dyn crate::cleanup::ReplayContext,
        function: &ResolvedFunction,
    ) -> bool {
        std::ptr::eq(self.program, program) && std::ptr::eq(self.function, function)
    }
    pub(super) fn inventory(&self) -> &crate::cleanup::CleanupInventory {
        &self.inventory
    }
}
fn fail(function: &ResolvedFunction) -> Diagnostic {
    replay_error(function, "budgeted borrowed helper profile is not complete")
}
fn copy_type(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    ty: &ResolvedType,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    budget.charge(function, 1, "borrowed helper Copy classification")?;
    if crate::hir::borrowed_scalar::option_u8::is_type(ty) {
        crate::hir::borrowed_scalar::option_u8::authenticated_metered(
            program.declarations(),
            ty,
            &mut |units| budget.charge(function, units, "borrowed helper exact Option receiver"),
        )
    } else {
        Ok(crate::hir::is_scalar_resolved_type(ty))
    }
}
fn id_equal(
    function: &ResolvedFunction,
    a: &str,
    b: &str,
    budget: &mut ReplayBudget,
) -> Result<bool, Diagnostic> {
    budget.charge(function, 1, "borrowed helper member identity length")?;
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.bytes().zip(b.bytes()) {
        budget.charge(function, 1, "borrowed helper member identity byte")?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
fn call(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
    callee: &crate::hir::DeclarationId,
    args: &[ResolvedExpr],
    budget: &mut ReplayBudget,
) -> Result<(), Diagnostic> {
    if crate::hir::borrowed_scalar::builtin(callee.as_str()) {
        return Ok(());
    }
    let mut target = None;
    for index in 0..program.function_count() {
        budget.charge(function, 1, "borrowed helper actual callee probe")?;
        let current = program.function_at(index).ok_or_else(|| fail(function))?;
        if id_equal(function, current.id.as_str(), callee.as_str(), budget)? {
            if target.replace(current).is_some() {
                return Err(fail(function));
            }
        }
    }
    let target = target.ok_or_else(|| fail(function))?;
    budget.charge(function, 1, "borrowed helper current callee header")?;
    if !crate::hir::is_scalar_resolved_type(&target.return_type)
        || !target.effects.is_empty()
        || target.yields.is_some()
        || args.len() != target.params.len()
    {
        return Err(fail(function));
    }
    for (argument, parameter) in args.iter().zip(&target.params) {
        budget.charge(function, 1, "borrowed helper actual callee parameter")?;
        if !crate::hir::borrowed_scalar::parameter(&parameter.ty, parameter.ownership)
            || argument.ty != parameter.ty
            || argument.ownership != parameter.ownership
        {
            return Err(fail(function));
        }
    }
    Ok(())
}
pub(super) fn build<'a>(
    program: &'a dyn crate::cleanup::ReplayContext,
    function: &'a ResolvedFunction,
    budget: &mut ReplayBudget,
) -> Result<Profile<'a>, Diagnostic> {
    budget.charge(function, 1, "borrowed helper current context header")?;
    if budget.physical.is_none()
        || !program.interfaces().is_empty()
        || !program.function_templates().is_empty()
        || !program.function_instances().is_empty()
        || !function.effects.is_empty()
        || function.yields.is_some()
        || !crate::hir::is_scalar_resolved_type(&function.return_type)
    {
        return Err(fail(function));
    }
    let mut found = false;
    for index in 0..program.function_count() {
        budget.charge(function, 1, "borrowed helper actual member probe")?;
        let member = program.function_at(index).ok_or_else(|| fail(function))?;
        if std::ptr::eq(member, function) {
            if found {
                return Err(fail(function));
            }
            found = true;
        }
    }
    if !found {
        return Err(fail(function));
    }
    for parameter in &function.params {
        budget.charge(function, 1, "borrowed helper parameter profile")?;
        if !crate::hir::borrowed_scalar::parameter(&parameter.ty, parameter.ownership) {
            return Err(fail(function));
        }
    }
    // Own-root absence is established by the complete typed walk below. The
    // attached empty loan proof is then a consistency guard, not its authority.
    if function.loan_plan.schema != crate::loan_plan::LOAN_PLAN_SCHEMA_V1
        || !function.loan_plan.loans.is_empty()
        || !function.loan_plan.endpoints.is_empty()
        || !function.loan_plan.edges.is_empty()
    {
        return Err(fail(function));
    }
    budget.reserve_array::<Frame<'_>>(
        function,
        DEPTH,
        "borrowed helper complete profile cursor",
    )?;
    let mut frames = [None; DEPTH];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        frames[0] = Some((crate::hir::lexical_exit::ChildCursor::new(root), false));
        let mut depth = 1;
        while depth != 0 {
            budget.charge(function, 1, "borrowed helper complete profile frame")?;
            let (mut cursor, entered) = frames[depth - 1].take().ok_or_else(|| fail(function))?;
            let expression = cursor.expression();
            if !entered {
                if !crate::hir::borrowed_scalar::expression(function, expression, &mut |units| {
                    budget.charge(function, units, "borrowed helper rooted operand")
                })? {
                    return Err(fail(function));
                }
                if crate::hir::borrowed_scalar::option_u8::is_type(&expression.ty)
                    && !copy_type(program, function, &expression.ty, budget)?
                {
                    return Err(fail(function));
                }
                match &expression.kind {
                    ResolvedExprKind::Int(_)
                    | ResolvedExprKind::Int32(_)
                    | ResolvedExprKind::Char(_)
                    | ResolvedExprKind::Uint8(_)
                    | ResolvedExprKind::Usize(_)
                    | ResolvedExprKind::Float32(_)
                    | ResolvedExprKind::Float64(_)
                    | ResolvedExprKind::Bool(_)
                    | ResolvedExprKind::Unary { .. }
                    | ResolvedExprKind::Binary { .. }
                    | ResolvedExprKind::If { .. }
                    | ResolvedExprKind::BorrowPlace { .. } => {}
                    ResolvedExprKind::Place(place) if place.projections.is_empty() => {}
                    ResolvedExprKind::Call {
                        callee,
                        instance: None,
                        type_arguments,
                        args,
                    } if type_arguments.is_empty() => {
                        call(program, function, callee, args, budget)?
                    }
                    ResolvedExprKind::Match { scrutinee, .. }
                        if crate::hir::borrowed_scalar::option_u8::is_type(&scrutinee.ty) => {}
                    ResolvedExprKind::Block { statements, .. } => {
                        for statement in statements {
                            budget.charge(
                                function,
                                1,
                                "borrowed helper complete statement profile",
                            )?;
                            match statement {
                                ResolvedStatement::Let { binding, .. }
                                | ResolvedStatement::Assign {
                                    binding,
                                    field: None,
                                    ..
                                } if binding.ownership == OwnershipMode::Value
                                    && copy_type(program, function, &binding.ty, budget)? => {}
                                ResolvedStatement::While { .. } => {}
                                _ => return Err(fail(function)),
                            }
                        }
                    }
                    _ => return Err(fail(function)),
                }
            }
            if let Some(child) = cursor.next() {
                if depth == DEPTH {
                    return Err(fail(function));
                }
                frames[depth - 1] = Some((cursor, true));
                frames[depth] = Some((crate::hir::lexical_exit::ChildCursor::new(child), false));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
    }
    // Temporarily move the sole backing owner so constructor work can debit the
    // active replay ledger. Restore it on every Result path; no credits return.
    let mut physical = budget.physical.take().ok_or_else(|| fail(function))?;
    let inventory = crate::cleanup::build_borrowed_helper_inventory_with_budget(
        program,
        function,
        &mut physical,
        &mut |units| budget.charge(function, units, "borrowed helper independent inventory"),
    );
    budget.physical = Some(physical);
    let inventory = inventory?;
    budget.reserve_array::<Profile<'_>>(function, 1, "borrowed helper held profile")?;
    Ok(Profile {
        program,
        function,
        inventory,
    })
}
#[cfg(test)]
mod tests;
