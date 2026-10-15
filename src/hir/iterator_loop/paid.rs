//! Complete iterator preflight with paid borrowed scratch.
use super::*;

pub(crate) fn validate_function_paid(
    function: &ResolvedFunction,
) -> Result<(), crate::diagnostic::Diagnostic> {
    let mut pending = crate::hir::ordinary_scan::Stack::new()?;
    pending.push(&function.body)?;
    let mut authorized = BTreeSet::new();
    let mut replacements = crate::hir::ordinary_scan::Stack::new()?;
    while let Some(expression) = pending.pop() {
        if let ResolvedExprKind::Block { statements, tail } = &expression.kind {
            for statement in statements {
                if let ResolvedStatement::Assign { binding, value, .. } = statement {
                    if crate::iterator_ops::is_step(&binding.ty) {
                        replacements.push(&value.id)?;
                    }
                }
                if let ResolvedStatement::While {
                    condition, body, ..
                }
                | ResolvedStatement::LexicalWhile {
                    condition, body, ..
                } = statement
                {
                    if let Some(protocol) = recognize(condition, body) {
                        let [ResolvedStatement::Let {
                            binding,
                            mutable: true,
                            value: seed,
                            ..
                        }, ResolvedStatement::While { .. }
                        | ResolvedStatement::LexicalWhile { .. }] = statements.as_slice()
                        else {
                            return Err(crate::diagnostic::Diagnostic::io_static(
                                "SPX-H006",
                                "iterator loop must retain one mutable initialized Step slot",
                            ));
                        };
                        if binding != protocol.step
                            || !zero(tail)
                            || !matches!(&seed.kind, ResolvedExprKind::Call { callee, type_arguments, instance: None, args }
                                if callee.as_str() == crate::iterator_ops::NEXT_ID
                                && matches!(args.as_slice(), [source] if source.ownership == OwnershipMode::Own && iter_matches(&source.ty, type_arguments.first().unwrap_or(&ResolvedType::Unit))))
                            || seed.ty != binding.ty
                            || seed.ownership != OwnershipMode::Own
                        {
                            return Err(crate::diagnostic::Diagnostic::io_static(
                                "SPX-H006",
                                "iterator loop seed, slot, or wrapper is inconsistent",
                            ));
                        }
                        let ResolvedExprKind::Block { statements, .. } = &body.kind else {
                            unreachable!()
                        };
                        let ResolvedStatement::Assign { value, .. } = &statements[0] else {
                            unreachable!()
                        };
                        crate::hir::ordinary_scan::insert(&mut authorized, &value.id)?;
                    }
                }
            }
        }
        crate::hir::ordinary_scan::children(expression, &mut |child| pending.push(child))?;
    }
    if replacements.iter().any(|id| !authorized.contains(id)) {
        return Err(crate::diagnostic::Diagnostic::io_static(
            "SPX-H006",
            "owning Step assignment is outside an authenticated consuming iterator loop",
        ));
    }
    Ok(())
}
