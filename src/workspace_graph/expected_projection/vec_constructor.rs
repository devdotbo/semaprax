//! Source markers must match current typed sites; an ordinary Block grants no v81 provenance.
use crate::workspace_graph::{graph_error, reserve_builder_structure, MAX_CALLS};
use crate::{ast, diagnostic::Diagnostic, hir};

fn refused() -> Vec<Diagnostic> {
    vec![graph_error(
        "SPX-G173",
        "Vec constructor source provenance disagrees with retained HIR",
    )]
}
struct Work(usize);
impl Work {
    fn debit(&mut self, bytes: usize) -> Result<(), Vec<Diagnostic>> {
        self.0 = self
            .0
            .checked_add(bytes)
            .filter(|n| *n <= MAX_CALLS)
            .ok_or_else(refused)?;
        Ok(())
    }
}
fn walk<'a>(
    roots: impl Iterator<Item = &'a hir::ResolvedExpr>,
    work: &mut Work,
    visit: &mut impl FnMut(&'a hir::ResolvedExpr, &mut Work) -> Result<(), Vec<Diagnostic>>,
) -> Result<(), Vec<Diagnostic>> {
    use hir::lexical_exit::ChildCursor;
    reserve_builder_structure(std::mem::size_of::<[Option<ChildCursor<'_>>; 514]>())?;
    let mut frames = [None; 514];
    for root in roots {
        frames[0] = Some(ChildCursor::new(root));
        let mut len = 1;
        while len != 0 {
            let cursor = frames[len - 1]
                .as_mut()
                .expect("retained constructor cursor");
            if !cursor.is_started() {
                work.debit(1)?;
                visit(cursor.expression(), work)?;
            }
            if let Some(child) = cursor.next() {
                if len == frames.len() {
                    return Err(refused());
                }
                frames[len] = Some(ChildCursor::new(child));
                len += 1;
            } else {
                len -= 1;
            }
        }
    }
    Ok(())
}

pub(super) fn verify(
    source: &ast::Program,
    resolved: &hir::ResolvedProgram,
) -> Result<(), Vec<Diagnostic>> {
    // Node-only presence uses the existing admitted source depth, with no
    // path, identity, or proof-vector allocation. HIR presence remains
    // independent so a typed-only forged marker cannot hide behind absence.
    fn source_presence(
        expression: &ast::Expr,
        depth: usize,
        work: &mut usize,
    ) -> Result<bool, Vec<Diagnostic>> {
        *work = work
            .checked_add(1)
            .filter(|n| *n <= super::super::MAX_TOTAL_SOURCE_BYTES)
            .ok_or_else(refused)?;
        if depth > 128 {
            return Err(refused());
        }
        if matches!(expression.kind, ast::ExprKind::VecConstructor { .. }) {
            return Ok(true);
        }
        let mut children = ast::vec_constructor::NodeCursor::new(expression);
        while let Some(child) = children.next() {
            if source_presence(child, depth + 1, work)? {
                return Ok(true);
            }
        }
        Ok(false)
    }
    let mut source_has_marker = false;
    let mut presence_work = 0usize;
    for function in &source.functions {
        for root in function
            .requires
            .iter()
            .chain(std::iter::once(&function.body))
            .chain(&function.ensures)
        {
            source_has_marker |= source_presence(root, 1, &mut presence_work)?;
        }
    }
    if !source_has_marker && !hir::vec_constructor::program_uses(resolved).map_err(|e| vec![e])? {
        return Ok(());
    }
    let mut work = Work(0);
    for function in &source.functions {
        let ordinary = resolved
            .functions
            .iter()
            .find(|f| f.id.as_str() == function.stable_id);
        let template = resolved
            .function_templates
            .iter()
            .find(|f| f.id.as_str() == function.stable_id);
        let (requires, body, ensures) = match (ordinary, template) {
            (Some(f), None) => (f.requires.as_slice(), &f.body, f.ensures.as_slice()),
            (None, Some(f)) => (f.requires.as_slice(), &f.body, f.ensures.as_slice()),
            _ => return Err(refused()),
        };
        let roots = || requires.iter().chain(std::iter::once(body)).chain(ensures);
        let mut sites = Vec::new();
        walk(roots(), &mut work, &mut |expression, _| {
            if matches!(
                expression.kind,
                hir::ResolvedExprKind::VecConstructor { .. }
            ) {
                hir::vec_constructor::authenticate(expression).map_err(|_| refused())?;
                if sites.len() == sites.capacity() {
                    let capacity = sites.capacity().checked_mul(2).unwrap_or(usize::MAX).max(4);
                    reserve_builder_structure(
                        capacity
                            .checked_mul(std::mem::size_of::<&hir::ResolvedExpr>())
                            .ok_or_else(refused)?,
                    )?;
                    sites
                        .try_reserve_exact(capacity - sites.len())
                        .map_err(|_| refused())?;
                    if sites.capacity() > capacity {
                        return Err(refused());
                    }
                }
                sites.push(expression);
            }
            Ok(())
        })?;
        let mut expected = 0usize;
        for (path, expression) in function
            .requires
            .iter()
            .enumerate()
            .map(|(i, e)| {
                (
                    crate::bounded_output::budgeted_format(format_args!("requires.{i}")),
                    e,
                )
            })
            .chain(std::iter::once((
                crate::bounded_output::budgeted_clone("body"),
                &function.body,
            )))
            .chain(function.ensures.iter().enumerate().map(|(i, e)| {
                (
                    crate::bounded_output::budgeted_format(format_args!("ensures.{i}")),
                    e,
                )
            }))
        {
            super::super::source_callables::visit_ast_expression_sites(
                expression,
                &path,
                &mut |expression, path| {
                    let ast::ExprKind::VecConstructor { .. } = &expression.kind else {
                        return Ok(());
                    };
                    work.debit(1)?;
                    expected += 1;
                    let expansion =
                        ast::vec_constructor::inspect(expression).map_err(|_| refused())?;
                    let identity =
                        hir::budgeted_workspace_expression_identity_text(&function.stable_id, path);
                    if crate::bounded_output::active_refused() {
                        return Err(refused());
                    }
                    let mut found = 0usize;
                    for typed in &sites {
                        work.debit(typed.id.as_str().len().max(identity.len()))?;
                        if typed.id.as_str() != identity {
                            continue;
                        }
                        let Some(typed_expansion) = hir::vec_constructor::expansion(typed) else {
                            return Err(refused());
                        };
                        if typed_expansion.count != expansion.count
                            || !same_type(
                                source,
                                function,
                                expansion.element,
                                typed_expansion.element,
                                0,
                                &mut work,
                            )?
                        {
                            return Err(refused());
                        }
                        found += 1;
                    }
                    if found != 1 {
                        return Err(refused());
                    }
                    Ok(())
                },
            )?;
        }
        if expected != sites.len() {
            return Err(refused());
        }
    }
    Ok(())
}

fn same_type(
    source: &ast::Program,
    function: &ast::Function,
    source_type: &ast::Type,
    typed: &hir::ResolvedType,
    depth: usize,
    work: &mut Work,
) -> Result<bool, Vec<Diagnostic>> {
    use ast::Type as A;
    use hir::ResolvedType as H;
    work.debit(1)?;
    if depth >= 129 {
        return Err(refused());
    }
    Ok(match (source_type, typed) {
        (A::I64, H::I64)
        | (A::I32, H::I32)
        | (A::Char, H::Char)
        | (A::U8, H::U8)
        | (A::Usize, H::Usize)
        | (A::F32, H::F32)
        | (A::F64, H::F64)
        | (A::Bool, H::Bool)
        | (A::String, H::String)
        | (A::Bytes, H::Bytes)
        | (A::Str, H::Str)
        | (A::SliceU8, H::SliceU8)
        | (A::StringMap, H::StringMap) => true,
        (A::ArrayU8(a), H::ArrayU8(b)) => a == b,
        (A::Named { name, arguments }, H::TypeParameter { owner, index }) => {
            owner.as_str() == function.stable_id
                && arguments.is_empty()
                && function
                    .type_parameters
                    .get(*index as usize)
                    .is_some_and(|p| p.name == *name)
        }
        (
            A::Named { name, arguments },
            H::Nominal {
                declaration,
                arguments: typed_args,
            },
        ) => {
            work.debit(name.len().max(declaration.as_str().len()))?;
            let target = source
                .types
                .iter()
                .find(|d| d.name == *name)
                .map(|d| d.stable_id.as_str())
                .or_else(|| {
                    source
                        .module_uses
                        .iter()
                        .find(|u| u.kind == ast::ModuleUseKind::Type && u.alias == *name)
                        .map(|u| u.persistent_id.as_str())
                })
                .or_else(|| {
                    (name == "Vec" && !source.types.iter().any(|d| d.name == "Vec"))
                        .then_some(crate::prelude::VEC_ID)
                });
            if target != Some(declaration.as_str()) || arguments.len() != typed_args.len() {
                false
            } else {
                let mut equal = true;
                for (a, b) in arguments.iter().zip(typed_args) {
                    equal &= same_type(source, function, a, b, depth + 1, work)?;
                }
                equal
            }
        }
        _ => false,
    })
}

#[cfg(test)]
mod tests {
    use crate::{ast, hir};
    const SOURCE: &str = "module constructor; @id(\"main\") fn main()->i64 {let rows=vec_of<i64>(19,23); if vec_len<i64>(rows)==2usize {42}else{0}}";
    fn marker(program: &mut hir::ResolvedProgram) -> &mut hir::ResolvedExpr {
        let hir::ResolvedExprKind::Block { statements, .. } = &mut program.functions[0].body.kind
        else {
            panic!()
        };
        let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
            panic!()
        };
        value
    }
    #[test]
    fn imported_nominal_constructor_type_sites_keep_the_replaced_block_identity() {
        let sources = [
            ("provider.spx", "module provider.rows; @id(\"row\") record Row { @id(\"row.value\") value:i64, } @id(\"provider.main\") fn main()->i64{0}"),
            ("caller.spx", "module app.main; use type @id(\"row\") from provider.rows as Row; @id(\"main\") fn main()->i64{let rows=vec_of<Row>(Row{value:42}); if vec_len<Row>(rows)==1usize {42}else{0}}"),
        ].into_iter().map(|(path, source)| crate::workspace_graph::WorkspaceSource {
            path: path.to_owned(),
            source: crate::format::canonical(&crate::parse(source, path).unwrap()),
        }).collect();
        let build = crate::workspace_graph::build_owned(sources).unwrap();
        let path = "body.s0.value.s0.value";
        let identity = hir::workspace_expression_identity(&hir::DeclarationId::new("main"), path);
        assert!(build.edges.iter().any(|edge| edge.kind == "type_reference"
            && edge.target == "row"
            && edge.ast_path == format!("{path}.type_argument.0")
            && edge.expression == identity));
    }

    #[test]
    fn retained_constructor_requires_exact_current_source_site_and_typed_marker_inventory() {
        let source = crate::check(SOURCE, "constructor.spx").unwrap();
        let typed = hir::resolve(&source).unwrap();
        hir::validate(&typed).unwrap();
        super::verify(&source, &typed).unwrap();
        let mut stale = typed.clone();
        marker(&mut stale).id = hir::ExpressionId::from_owned("foreign.constructor".to_owned());
        assert_eq!(
            super::verify(&source, &stale).unwrap_err()[0].code,
            "SPX-G173"
        );
        let mut ordinary = typed.clone();
        let value = marker(&mut ordinary);
        let hir::ResolvedExprKind::VecConstructor {
            statements, tail, ..
        } = &mut value.kind
        else {
            panic!()
        };
        value.kind = hir::ResolvedExprKind::Block {
            statements: std::mem::take(statements),
            tail: tail.clone(),
        };
        assert_eq!(
            super::verify(&source, &ordinary).unwrap_err()[0].code,
            "SPX-G173"
        );
        let mut unmarked = source.clone();
        let ast::ExprKind::Block { statements, .. } = &mut unmarked.functions[0].body.kind else {
            panic!()
        };
        let ast::Statement::Let { value, .. } = &mut statements[0] else {
            panic!()
        };
        let ast::ExprKind::VecConstructor {
            value: expansion, ..
        } = &mut value.kind
        else {
            panic!()
        };
        *value = (**expansion).clone();
        assert_eq!(
            super::verify(&unmarked, &typed).unwrap_err()[0].code,
            "SPX-G173"
        );
    }
}
