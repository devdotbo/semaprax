//! Cross-file type references introduced by authenticated row-field lowering.
use super::*;

pub(super) fn collect_edges(
    program: &Program,
    programs: &[Program],
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    let expected = super::super::record_traversal::collect(program, programs)?;
    for region in &expected.loops {
        push(
            program,
            region.function,
            region.declaration,
            &budgeted_format(format_args!("{}.s0.value.type_argument.0", region.path)),
            &budgeted_format(format_args!("{}.s0.value", region.path)),
            module_paths,
            authored,
            edges,
        )?;
    }
    for read in &expected.reads {
        let type_path = if read.copy {
            budgeted_format(format_args!("{}.base.type_argument.0", read.path))
        } else if read.bytes {
            budgeted_format(format_args!("{}.arg.0.type_argument.0", read.path))
        } else {
            budgeted_format(format_args!("{}.type_argument.0", read.path))
        };
        let expression_path = if read.copy {
            budgeted_format(format_args!("{}.base", read.path))
        } else {
            crate::bounded_output::budgeted_clone(&read.path)
        };
        push(
            program,
            read.function,
            read.declaration,
            &type_path,
            &expression_path,
            module_paths,
            authored,
            edges,
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn push(
    program: &Program,
    function: &Function,
    declaration: &TypeDeclaration,
    type_path: &str,
    expression_path: &str,
    module_paths: &BTreeMap<&str, &str>,
    authored: &BTreeMap<&str, AuthoredDeclaration<'_>>,
    edges: &mut Vec<WorkspaceEdge>,
) -> Result<(), Vec<Diagnostic>> {
    // A factory's defining module can use a different local spelling. Only
    // the consumer's explicit persistent type import owns this edge.
    let Some(module_use) = program.module_uses.iter().find(|item| {
        item.kind == ModuleUseKind::Type && item.persistent_id == declaration.stable_id
    }) else {
        return Ok(());
    };
    let target = authored
        .get(module_use.persistent_id.as_str())
        .ok_or_else(|| {
            vec![graph_error(
                "SPX-G173",
                "record traversal imported layout is missing",
            )]
        })?;
    push_edge(
        edges,
        WorkspaceEdge {
            caller_path: crate::bounded_output::budgeted_clone(&program.path),
            caller: crate::bounded_output::budgeted_clone(&function.stable_id),
            target_path: crate::bounded_output::budgeted_clone(module_paths[target.module]),
            target: crate::bounded_output::budgeted_clone(&module_use.persistent_id),
            kind: "type_reference",
            site: "type",
            expression: hir::workspace_expression_identity(
                &hir::DeclarationId::new(&function.stable_id),
                expression_path,
            ),
            ast_path: crate::bounded_output::budgeted_clone(type_path),
            alias: crate::bounded_output::budgeted_clone(&module_use.alias),
            ordinal: edges.len(),
        },
    )
}
