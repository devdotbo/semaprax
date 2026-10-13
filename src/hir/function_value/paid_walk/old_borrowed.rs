use super::*;
pub(crate) fn validate_uses(program: &ResolvedProgram) -> Result<(), Diagnostic> {
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
    {
        let mut sites = BTreeSet::new();
        let mut bindings = BTreeMap::new();
        let mut invalid = false;
        super::super::function_value::walk(function, |expression| {
            if is_borrowed(expression) {
                sites.insert(expression.id.clone());
            }
            if let ResolvedExprKind::Block { statements, .. }
            | ResolvedExprKind::VecConstructor { statements, .. } = &expression.kind
            {
                for statement in statements {
                    if let ResolvedStatement::Let {
                        binding,
                        mutable,
                        value,
                        ..
                    } = statement
                    {
                        if is_borrowed(value) {
                            invalid |= *mutable;
                            bindings.insert(binding.id.clone(), value.id.clone());
                            let ResolvedExprKind::Closure { captures, .. } = &value.kind else {
                                unreachable!()
                            };
                            for capture in captures {
                                let ResolvedExprKind::Place(root) = &capture.value.kind else {
                                    invalid = true;
                                    continue;
                                };
                                invalid |= !function.params.iter().any(|p| {
                                    p.id == root.root
                                        && p.ty == ResolvedType::Str
                                        && p.ownership == OwnershipMode::Borrow
                                });
                            }
                        }
                    }
                }
            }
        });
        if sites.is_empty() {
            continue;
        }
        invalid |= !function.effects.is_empty()
            || function.yields.is_some()
            || sites != bindings.values().cloned().collect();
        let mut contracts = function
            .requires
            .iter()
            .chain(&function.ensures)
            .collect::<Vec<_>>();
        while let Some(expression) = contracts.pop() {
            invalid |= is_borrowed(expression);
            super::super::push_resolved_expression_children_in_authored_order(
                expression,
                &mut contracts,
            );
        }
        let mut direct_invocations = BTreeSet::new();
        super::super::function_value::walk(function, |expression| {
            if let ResolvedExprKind::Invoke { callable, .. } = &expression.kind {
                if matches!(&callable.kind, ResolvedExprKind::Place(place) if bindings.contains_key(&place.root) && place.projections.is_empty())
                {
                    direct_invocations.insert(callable.id.clone());
                }
            }
        });
        super::super::function_value::walk(function, |expression| {
            if let ResolvedExprKind::Place(place) = &expression.kind {
                if bindings.contains_key(&place.root)
                    && !direct_invocations.contains(&expression.id)
                {
                    invalid = true;
                }
            }
        });
        // Generic instances cannot acquire this profile by forged substitution.
        invalid |= program
            .function_instances
            .iter()
            .any(|i| i.function.id == function.id);
        if invalid {
            return Err(hir_error(
                "borrowed closure must remain in its parameter-rooted synchronous local scope",
            ));
        }
    }
    Ok(())
}
