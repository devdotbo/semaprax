//! Independent source-derived synthetic callee replay at exact source paths.
use super::*;
pub(super) fn verify(
    program: &Program,
    resolved: &hir::ResolvedProgram,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    let fail = || {
        vec![graph_error(
            "SPX-G173",
            "explicit record sort source descriptor disagrees with retained HIR",
        )]
    };
    let mut expected = Vec::new();
    for function in &program.functions {
        let owner = hir::DeclarationId::new(&function.stable_id);
        for (root, expression) in function
            .requires
            .iter()
            .enumerate()
            .map(|(i, e)| (budgeted_format(format_args!("requires.{i}")), e))
            .chain(std::iter::once((String::from("body"), &function.body)))
            .chain(
                function
                    .ensures
                    .iter()
                    .enumerate()
                    .map(|(i, e)| (budgeted_format(format_args!("ensures.{i}")), e)),
            )
        {
            super::super::source_callables::visit_ast_call_expressions(
                expression,
                &root,
                &mut |expr, path| {
                    let ExprKind::Call {
                        name,
                        type_arguments,
                        args,
                    } = &expr.kind
                    else {
                        unreachable!()
                    };
                    if name != crate::vec_ops::sort_keys::NAME {
                        return Ok(());
                    }
                    let [Type::Named { name, arguments }] = type_arguments.as_slice() else {
                        return Err(fail());
                    };
                    if !arguments.is_empty() {
                        return Err(fail());
                    }
                    let [_, literal] = args.as_slice() else {
                        return Err(fail());
                    };
                    let ExprKind::String(keys) = &literal.kind else {
                        return Err(fail());
                    };
                    let local = program.types.iter().find(|ty| ty.name == *name);
                    let declaration = local
                        .or_else(|| {
                            program
                                .module_uses
                                .iter()
                                .find(|item| {
                                    item.kind == ModuleUseKind::Type && item.alias == *name
                                })
                                .and_then(|item| authored.get(item.persistent_id.as_str()))
                                .and_then(|item| item.ty)
                        })
                        .ok_or_else(fail)?;
                    let descriptor = crate::vec_ops::sort_keys::source(declaration, keys)
                        .and_then(|d| d.identity())
                        .ok_or_else(fail)?;
                    reserve_builder_structure(
                        std::mem::size_of::<(String, String, String)>()
                            + function.stable_id.len()
                            + descriptor.len(),
                    )?;
                    expected.push((
                        function.stable_id.clone(),
                        hir::workspace_expression_identity(&owner, path),
                        descriptor,
                    ));
                    Ok(())
                },
            )?;
        }
    }
    let mut actual = Vec::new();
    hir::visit_workspace_call_sites(resolved, &mut |owner, expression, target| {
        if target
            .as_str()
            .starts_with(crate::vec_ops::sort_keys::PREFIX)
        {
            reserve_builder_structure(
                std::mem::size_of::<(String, String, String)>()
                    + owner.as_str().len()
                    + expression.len()
                    + target.as_str().len(),
            )?;
            actual.push((
                owner.as_str().to_owned(),
                expression.to_owned(),
                target.as_str().to_owned(),
            ));
        }
        Ok::<(), Vec<Diagnostic>>(())
    })?;
    expected.sort();
    actual.sort();
    if expected != actual {
        return Err(fail());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn independent_source_projection_binds_literal_keys_at_exact_call_identity() {
        let source = include_str!("../../../tests/fixtures/explicit-copy-sort-keys.spx");
        let ast = crate::check(source, "rank.spx").unwrap();
        let mut resolved = hir::resolve(&ast).unwrap();
        verify(&ast, &resolved, &BTreeMap::new()).unwrap();
        let function = resolved
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
            panic!()
        };
        let hir::ResolvedStatement::Assign { value, .. } = &mut statements[5] else {
            panic!()
        };
        let hir::ResolvedExprKind::Call { callee, .. } = &mut value.kind else {
            panic!()
        };
        let mut descriptor = crate::vec_ops::sort_keys::decode(callee.as_str()).unwrap();
        descriptor.keys[0].descending = false;
        *callee = hir::DeclarationId::new(descriptor.identity().unwrap());
        // It is independently valid HIR for a DIFFERENT policy, but not this source.
        hir::validate(&resolved).unwrap();
        assert!(verify(&ast, &resolved, &BTreeMap::new()).is_err());
    }
    #[test]
    fn imported_nominal_authority_reconstructs_same_descriptor_and_rejects_prefix_import() {
        use crate::workspace_graph::{index_authored, synthetic_program};
        let provider=crate::parse("module rows; @id(\"rows.r\") record R { @id(\"rows.key\") key:i64, } @id(\"rows.main\") fn main()->i64{0}","rows.spx").unwrap();
        let caller=crate::parse("module app; use type @id(\"rows.r\") from rows as R; @id(\"app.main\") fn main()->i64{let v=vec_with_capacity<R>(0usize);let w=vec_sort_by<R>(v,\"key:desc\");0}","app.spx").unwrap();
        let programs = vec![provider, caller];
        let authored = index_authored(&programs).unwrap();
        let synthetic = synthetic_program(&programs[1], &authored, &programs).unwrap();
        let resolved =
            crate::vec_ops::with_authenticated_linked_source(|| hir::resolve(&synthetic)).unwrap();
        verify(&programs[1], &resolved, &authored).unwrap();
        let hostile=crate::parse("module bad; use function @id(\"core.vec.sort-by.v1:forged\") from rows as forged; @id(\"bad.main\") fn main()->i64{0}","bad.spx").unwrap();
        let errors = crate::workspace_graph::validate_uses(
            &[hostile],
            &BTreeMap::from([("rows", "rows.spx")]),
            &authored,
        )
        .unwrap_err();
        assert!(errors
            .iter()
            .any(|d| d.message.contains("cannot be imported")));
    }
}
