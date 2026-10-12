//! Complete pre-change paid argument receiver with its saved predicate.
use super::{record_vec_borrow, OwnershipMode, ResolvedExpr, ResolvedExprKind};

pub(super) fn borrowed_call_arguments_paid<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    expression: &'a ResolvedExpr,
    payer: Option<&crate::bounded_output::cfg_reservation::CfgReservation>,
) -> Result<Vec<(usize, &'a ResolvedExpr)>, crate::diagnostic::Diagnostic> {
    let mut out = Vec::new();
    match &expression.kind {
        ResolvedExprKind::VecFieldRead { args, .. } | ResolvedExprKind::RowLoan { args, .. } => {
            if let Some(source) = args.first() {
                super::storage::push(&mut out, (0, source), payer)?;
            }
        }
        ResolvedExprKind::Call {
            callee,
            instance,
            args,
            ..
        } => {
            let target = program.resolve_call_target(callee, instance.as_ref());
            for (index, argument) in args.iter().enumerate() {
                if (super::paid_predicates_legacy::string_slot(expression).is_some() && index == 0)
                    || crate::vec_ops::field_unique::borrowed_source(expression, index)
                    || record_vec_borrow::argument(program, expression, index, argument)
                    || match target {
                        Some(target) => target
                            .params
                            .get(index)
                            .is_some_and(|parameter| parameter.ownership == OwnershipMode::Borrow),
                        None => {
                            argument.ownership == OwnershipMode::Borrow
                                || match payer {
                                    Some(payer) => {
                                        super::paid_predicates_legacy::borrowed_vec_argument(
                                            program, expression, index, argument, payer,
                                        )?
                                    }
                                    None => {
                                        crate::hir::owned_collection_record::borrowed_vec_argument(
                                            program, expression, index, argument,
                                        )
                                    }
                                }
                        }
                    }
                {
                    super::storage::push(&mut out, (index, argument), payer)?;
                }
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            let target = program
                .interfaces()
                .iter()
                .flat_map(|i| &i.imports)
                .find(|i| i.id == call.import);
            for (index, argument) in call.args.iter().enumerate() {
                if target.is_some_and(|i| {
                    i.parameters
                        .get(index)
                        .is_some_and(|parameter| parameter.ownership == OwnershipMode::Borrow)
                }) {
                    super::storage::push(&mut out, (index, argument), payer)?;
                }
            }
        }
        _ => {}
    }
    Ok(out)
}
