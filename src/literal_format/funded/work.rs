//! Test-only work-metered twin of `checked_moving_string_operands_funded`.
//! Precedence is the original's: same-owner concat operands first, then every
//! `take` operand and every direct owning String Place passed to a
//! `LiteralFormat`, in walk order. Each argument test is announced before it
//! runs; insertions use the metered set receiver; the operand set's Drop is
//! paid when a later step fails.
use super::*;
use crate::hir::funded::work::{self as paid, Work};

pub(crate) fn checked_moving_string_operands_funded_with_work<E>(
    function: &crate::hir::ResolvedFunction,
    work: Work<'_, E>,
    reserve: Reserve<'_, E>,
) -> Result<BTreeSet<crate::hir::ExpressionId>, FundedError<E>> {
    use crate::hir::{OwnershipMode, ResolvedExprKind, ResolvedType};
    let mut operands =
        crate::string_ops::same_owner_concat_operands_funded_with_work(function, work, reserve)?;
    let walked = crate::hir::function_value::walk_funded_paying_with_work(
        function,
        |expression, work, reserve| {
            paid::tick(work, 1)?;
            // REQUIRED source (section 4): work twin of `take::operand`.
            if let Some(argument) = crate::string_ops::take::operand_with_work(expression, work)? {
                paid::insert_cloned_key_with_work(&mut operands, &argument.id, work, reserve)?;
            }
            if let ResolvedExprKind::LiteralFormat { args, .. } = &expression.kind {
                for argument in args {
                    paid::tick(work, 1)?;
                    if argument.ty == ResolvedType::String {
                        paid::tick(work, 1)?;
                        if argument.ownership == OwnershipMode::Own {
                            paid::tick(work, 1)?;
                            if matches!(argument.kind, ResolvedExprKind::Place(_)) {
                                paid::insert_cloned_key_with_work(
                                    &mut operands,
                                    &argument.id,
                                    work,
                                    reserve,
                                )?;
                            }
                        }
                    }
                }
            }
            Ok(())
        },
        work,
        reserve,
    );
    match walked {
        Ok(()) => Ok(operands),
        Err(error) => Err(paid::abandon(operands, error, work)),
    }
}
