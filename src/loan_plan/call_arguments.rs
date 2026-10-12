use super::{record_vec_borrow, OwnershipMode, ResolvedExpr, ResolvedExprKind};

// Native loans use the same exact place origins and CFG interval as ordinary
// borrowed calls. The declared parameter mode is authoritative; a named owned
// argument remains an owned Place node while the invocation borrows it.
pub(super) fn borrowed_call_arguments<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    expression: &'a ResolvedExpr,
) -> Vec<(usize, &'a ResolvedExpr)> {
    match &expression.kind {
        // A Row Loan v1 borrows its whole Vec place exactly like a scoped field
        // read; its enclosing call adds the call-long loan of that same origin.
        ResolvedExprKind::VecFieldRead { args, .. } | ResolvedExprKind::RowLoan { args, .. } => {
            args.first()
                .map(|source| vec![(0, source)])
                .unwrap_or_default()
        }
        ResolvedExprKind::Call {
            callee,
            instance,
            args,
            ..
        } => {
            let target = program.resolve_call_target(callee, instance.as_ref());
            args.iter()
                .enumerate()
                .filter(|(index, argument)| {
                    (crate::vec_ops::string_slot::inspect(expression).is_some() && *index == 0)
                        || crate::vec_ops::field_unique::borrowed_source(expression, *index)
                        || record_vec_borrow::argument(program, expression, *index, argument)
                        || target.map_or_else(
                            || {
                                argument.ownership == OwnershipMode::Borrow
                                    || crate::hir::owned_collection_record::borrowed_vec_argument(
                                        program, expression, *index, argument,
                                    )
                            },
                            |target| {
                                target
                                    .params
                                    .get(*index)
                                    .is_some_and(|p| p.ownership == OwnershipMode::Borrow)
                            },
                        )
                })
                .collect()
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            let target = program
                .interfaces()
                .iter()
                .flat_map(|i| &i.imports)
                .find(|i| i.id == call.import);
            call.args
                .iter()
                .enumerate()
                .filter(|(index, _)| {
                    target.is_some_and(|i| {
                        i.parameters
                            .get(*index)
                            .is_some_and(|p| p.ownership == OwnershipMode::Borrow)
                    })
                })
                .collect()
        }
        _ => Vec::new(),
    }
}

// Same filters and left-to-right result order, with every actual replacement
// funded before its tuple is appended. The public legacy receiver above is
// retained unchanged for independent callers and saved planner comparison.
pub(super) fn borrowed_call_arguments_paid<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    expression: &'a ResolvedExpr,
    payer: Option<&crate::bounded_output::cfg_reservation::CfgReservation>,
) -> Result<Vec<(usize, &'a ResolvedExpr)>, crate::diagnostic::Diagnostic> {
    let mut out = Vec::new();
    match &expression.kind {
        ResolvedExprKind::VecFieldRead { args, .. } | ResolvedExprKind::RowLoan { args, .. } => {
            if let Some(source) = args.first() { super::storage::push(&mut out, (0, source), payer)?; }
        }
        ResolvedExprKind::Call { callee, instance, args, .. } => {
            let target = program.resolve_call_target(callee, instance.as_ref());
            for (index, argument) in args.iter().enumerate() {
                if (super::paid_predicates::string_slot(expression).is_some() && index == 0)
                    || crate::vec_ops::field_unique::borrowed_source(expression, index)
                    || record_vec_borrow::argument(program, expression, index, argument)
                    || target.map_or_else(
                        || argument.ownership == OwnershipMode::Borrow
                            || crate::hir::owned_collection_record::borrowed_vec_argument(
                                program, expression, index, argument),
                        |target| target.params.get(index)
                            .is_some_and(|parameter| parameter.ownership == OwnershipMode::Borrow))
                {
                    super::storage::push(&mut out, (index, argument), payer)?;
                }
            }
        }
        ResolvedExprKind::NativeRustImportCall(call) => {
            let target = program.interfaces().iter().flat_map(|i| &i.imports)
                .find(|i| i.id == call.import);
            for (index, argument) in call.args.iter().enumerate() {
                if target.is_some_and(|i| i.parameters.get(index)
                    .is_some_and(|parameter| parameter.ownership == OwnershipMode::Borrow))
                {
                    super::storage::push(&mut out, (index, argument), payer)?;
                }
            }
        }
        _ => {}
    }
    Ok(out)
}
