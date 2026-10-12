//! The pre-checked Vec-only purity classifier, retained verbatim for tests as
//! an independent oracle. It never calls the generic implementation it audits.
use super::super::*;
mod borrowed_records;
pub(super) fn pure_profile_metered(
    function: &ResolvedFunction,
    work: &mut usize,
    charge: &mut dyn FnMut(usize) -> Result<(), Vec<Diagnostic>>,
    option_shape: bool,
    records: &[hir::ResolvedTypeDeclaration],
) -> Result<bool, Vec<Diagnostic>> {
    let mut take = |units| -> Result<(), Vec<Diagnostic>> {
        let remaining = work.checked_sub(units).ok_or_else(fail)?;
        charge(units)?;
        *work = remaining;
        Ok(())
    };
    take(function.params.len().checked_add(1).ok_or_else(fail)?)?;
    if !function.effects.is_empty()
        || function.yields.is_some()
        || !(hir::borrowed_scalar::signature(function)
            || borrowed_records::signature(function, records, &mut take)?)
    {
        return Ok(false);
    }
    // The charged whitelist rejects every lexical statement and expression;
    // no separate unmetered lexical traversal is needed for this closed shape.
    let mut stack = [None; DEPTH];
    for root in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        stack[0] = Some(hir::lexical_exit::ChildCursor::new(root));
        let mut depth = 1;
        while depth != 0 {
            take(1)?;
            let mut cursor = stack[depth - 1].take().ok_or_else(fail)?;
            let expr = cursor.expression();
            if !cursor.is_started() {
                if (!option_shape
                    && (hir::borrowed_scalar::option_u8::is_type(&expr.ty)
                        || matches!(expr.kind, ResolvedExprKind::Match { .. })))
                    || !match borrowed_records::expression(function, expr, records, &mut take)? {
                        Some(admitted) => admitted,
                        None => hir::borrowed_scalar::expression(function, expr, &mut take)?,
                    }
                {
                    return Ok(false);
                }
                match &expr.kind {
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
                    | ResolvedExprKind::If { .. } => {}
                    // The receiver above has authenticated both bare scalar roots and
                    // exact scalar Field projections of current Borrow records.
                    ResolvedExprKind::Place(_) => {}
                    ResolvedExprKind::Project { base, .. }
                        if matches!(base.ty, ResolvedType::Nominal { .. }) => {}
                    ResolvedExprKind::BorrowPlace { .. } => {}
                    ResolvedExprKind::Match { scrutinee, .. }
                        if option_shape
                            && hir::borrowed_scalar::option_u8::is_type(&scrutinee.ty) => {}
                    ResolvedExprKind::Call {
                        instance,
                        type_arguments,
                        ..
                    } if instance.is_none() && type_arguments.is_empty() => {}
                    ResolvedExprKind::Block { statements, .. } => {
                        take(statements.len())?;
                        for statement in statements {
                            match statement {
                                ResolvedStatement::Let { binding, .. }
                                | ResolvedStatement::Assign {
                                    binding,
                                    field: None,
                                    ..
                                } if binding.ownership == OwnershipMode::Value
                                    && (hir::is_scalar_resolved_type(&binding.ty)
                                        || (option_shape
                                            && hir::borrowed_scalar::option_u8::is_type(
                                                &binding.ty,
                                            ))) => {}
                                ResolvedStatement::While { .. } => {}
                                _ => return Ok(false),
                            }
                        }
                    }
                    _ => return Ok(false),
                }
            }
            if let Some(child) = cursor.next() {
                if depth == DEPTH {
                    return Ok(false);
                }
                stack[depth - 1] = Some(cursor);
                stack[depth] = Some(hir::lexical_exit::ChildCursor::new(child));
                depth += 1;
            } else {
                depth -= 1;
            }
        }
    }
    Ok(true)
}
