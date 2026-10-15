//! Independent source-derived synthetic callee replay at exact source paths.
use crate::ast::{Expr, ExprKind, ModuleUseKind, Program, Type};
use crate::bounded_output::budgeted_format;
use crate::diagnostic::Diagnostic;
use crate::hir;
use crate::workspace_graph::{graph_error, reserve_builder_structure, AuthoredDeclaration};
use std::collections::BTreeMap;
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
    // A complete path-free census proves absence before constructing source
    // identities. Check HIR independently too: injected synthetic sort calls
    // must still reach the exact source/HIR comparison below.
    let mut source_sort = false;
    for function in &program.functions {
        for expression in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            source_sort |= contains_sort(expression);
        }
    }
    let resolved_sort = resolved.functions.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(contains_resolved_sort)
    }) || resolved.function_templates.iter().any(|function| {
        function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
            .any(contains_resolved_sort)
    });
    if !source_sort && !resolved_sort {
        return Ok(());
    }
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

fn contains_sort(expression: &Expr) -> bool {
    // One indexed frame per ancestor; width creates neither paths nor heap
    // scratch. Excess depth takes the full verifier rather than proving absence.
    let mut stack = [None; 514];
    stack[0] = Some((expression, 0usize));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (expression, next) = stack[len].take().expect("sort census frame");
        if next == 0
            && matches!(&expression.kind, ExprKind::Call { name, .. } if name == crate::vec_ops::sort_keys::NAME)
        {
            return true;
        }
        if let Some(child) = expression.child(next) {
            if len + 2 > stack.len() {
                return true;
            }
            stack[len] = Some((expression, next + 1));
            stack[len + 1] = Some((child, 0));
            len += 2;
        }
    }
    false
}

fn contains_resolved_sort(expression: &hir::ResolvedExpr) -> bool {
    let mut stack = [None; 514];
    stack[0] = Some((expression, 0usize));
    let mut len = 1usize;
    while len != 0 {
        len -= 1;
        let (expression, next) = stack[len].take().expect("resolved sort census frame");
        if next == 0
            && matches!(&expression.kind,
            hir::ResolvedExprKind::Call { callee: target, .. }
                | hir::ResolvedExprKind::FunctionReference { target }
                if target.as_str().starts_with(crate::vec_ops::sort_keys::PREFIX))
        {
            return true;
        }
        if let Some(child) = hir::lexical_exit::child(expression, next) {
            if len + 2 > stack.len() {
                return true;
            }
            stack[len] = Some((expression, next + 1));
            stack[len + 1] = Some((child, 0));
            len += 2;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::verify;
    use crate::hir;
    use std::collections::BTreeMap;
    #[test]
    fn absent_sort_census_allocates_no_paths_but_injected_sort_still_refuses() {
        let ast = crate::check(
            "module empty; @id(\"app.main\") fn main()->i64{let value=3;value+4}",
            "empty.spx",
        )
        .unwrap();
        let resolved = hir::resolve(&ast).unwrap();
        let (result, overflow, bytes) = crate::bounded_output::with_limit_usage(0, || {
            verify(&ast, &resolved, &BTreeMap::new())
        });
        result.unwrap();
        assert!(!overflow);
        assert_eq!(bytes, 0);
        let sorted = crate::check(
            include_str!("../../../tests/fixtures/explicit-copy-sort-keys.spx"),
            "rank.spx",
        )
        .unwrap();
        let sorted = hir::resolve(&sorted).unwrap();
        assert_eq!(
            verify(&ast, &sorted, &BTreeMap::new()).unwrap_err()[0].code,
            "SPX-G173"
        );
    }
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
