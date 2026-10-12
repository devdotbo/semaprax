//! Selective Graph v84 makes the no-result lexical statement boundary explicit.
use super::{BTreeSet, DeclarationId, Diagnostic, GraphView, ResolvedProgram};
const SCHEMA: &str = "semaprax.graph.v84";
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if !crate::hir::copy_discard_program_uses(program) {
        return super::byte_traversal::graph_schema(program);
    }
    crate::hir::validate(program)?;
    crate::loan_plan::validate_program(program)?;
    crate::hir::analyze_byte_data_capacity(program)?;
    Ok(SCHEMA)
}
pub(crate) fn legacy_graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if crate::hir::copy_discard_program_uses(program) {
        return Err(Diagnostic::io(
            "SPX-G410",
            "lexical Copy discard requires Graph v84",
        ));
    }
    super::byte_traversal::legacy_graph_schema(program)
}
pub(super) fn graph_json(
    program: &ResolvedProgram,
    revision: &str,
    functions: &BTreeSet<DeclarationId>,
    types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    let mut graph = super::byte_traversal::graph_json(program, revision, functions, types, view)?;
    if !crate::hir::copy_discard_program_uses(program) {
        return Ok(graph);
    }
    graph_schema(program)?;
    let inner = super::byte_traversal::graph_schema(program)?;
    let prefix = crate::bounded_output::budgeted_format(format_args!("{{\"schema\":\"{inner}\""));
    if !graph.starts_with(&prefix) {
        return Err(Diagnostic::io(
            "SPX-G410",
            "noncanonical lexical Copy discard graph",
        ));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v84\"");
    Ok(graph)
}
