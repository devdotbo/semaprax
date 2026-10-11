use super::{record_vec_borrow, OwnershipMode, ResolvedExpr, ResolvedExprKind};

// Native loans use the same exact place origins and CFG interval as ordinary
// borrowed calls. The declared parameter mode is authoritative; a named owned
// argument remains an owned Place node while the invocation borrows it.
pub(super) fn borrowed_call_arguments<'a>(
    program: &dyn crate::cleanup::ReplayContext,
    expression: &'a ResolvedExpr,
) -> Vec<(usize, &'a ResolvedExpr)> {
    match &expression.kind {
        ResolvedExprKind::VecFieldRead { args, .. } => args
            .first()
            .map(|source| vec![(0, source)])
            .unwrap_or_default(),
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
