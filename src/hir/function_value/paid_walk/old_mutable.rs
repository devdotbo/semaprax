use super::*;
pub(crate) fn validate_uses(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    use std::collections::BTreeSet;
    for function in program.functions.iter().chain(
        program
            .function_instances
            .iter()
            .map(|instance| &instance.function),
    ) {
        if function.params.iter().any(|p| p.ty.is_mut_function()) {
            return Err(hir_error("mutable callback parameters are not admitted"));
        }
        let mut receiver_ids = BTreeSet::new();
        let mut invalid = false;
        super::super::function_value::walk(function, |expression| {
            if let ResolvedExprKind::Invoke { callable, args } = &expression.kind {
                if callable.ty.is_mut_function() {
                    receiver_ids.insert(callable.id.clone());
                    let ResolvedExprKind::Place(receiver) = &callable.kind else {
                        invalid = true;
                        return;
                    };
                    let mut pending = args.iter().collect::<Vec<_>>();
                    while let Some(argument) = pending.pop() {
                        if let ResolvedExprKind::Invoke { callable, .. } = &argument.kind {
                            if matches!(&callable.kind, ResolvedExprKind::Place(place) if place.root == receiver.root)
                            {
                                invalid = true;
                            }
                        }
                        super::super::push_resolved_expression_children_in_authored_order(
                            argument,
                            &mut pending,
                        );
                    }
                }
            }
        });
        super::super::function_value::walk(function, |expression| {
            if expression.ty.is_mut_function()
                && matches!(expression.kind, ResolvedExprKind::Place(_))
                && !receiver_ids.contains(&expression.id)
            {
                invalid = true;
            }
        });
        if invalid {
            return Err(hir_error(
                "mutable receiver cannot be copied, escape, or reenter during argument staging",
            ));
        }
    }
    Ok(())
}
