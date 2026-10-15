//! The original in-loop walk, with direct borrowed children and paid spill.
use super::*;

pub(crate) fn function_has_record_renewal_outside_loop_paid(
    program: &dyn crate::cleanup::ReplayContext,
    function: &ResolvedFunction,
) -> Result<bool, Diagnostic> {
    let mut pending = crate::hir::ordinary_scan::Stack::new()?;
    pending.push((&function.body, false))?;
    while let Some((expression, in_loop)) = pending.pop() {
        if let ResolvedExprKind::Block { statements, tail } = &expression.kind {
            pending.push((tail, in_loop))?;
            for statement in statements {
                match statement {
                    ResolvedStatement::Let { value, .. } => pending.push((value, in_loop))?,
                    ResolvedStatement::Assign {
                        binding,
                        field: None,
                        value,
                        ..
                    } => {
                        if !in_loop && is_record_owner_renewal(program, binding, value) {
                            return Ok(true);
                        }
                        pending.push((value, in_loop))?;
                    }
                    ResolvedStatement::Assign { value, .. }
                    | ResolvedStatement::CopyDiscard { value, .. } => {
                        pending.push((value, in_loop))?
                    }
                    ResolvedStatement::Unsafe { body, .. } => pending.push((body, in_loop))?,
                    ResolvedStatement::While {
                        condition, body, ..
                    }
                    | ResolvedStatement::LexicalWhile {
                        condition, body, ..
                    } => {
                        pending.push((condition, in_loop))?;
                        pending.push((body, true))?;
                    }
                }
            }
        } else {
            crate::hir::ordinary_scan::children(expression, &mut |child| {
                pending.push((child, in_loop))
            })?;
        }
    }
    Ok(false)
}
