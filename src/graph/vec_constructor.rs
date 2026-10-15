//! Selective v81 constructors retain every inner v80/v79/v78/v77 fact.
use super::{BTreeSet, DeclarationId, Diagnostic, GraphView, ResolvedProgram};
const SCHEMA: &str = "semaprax.graph.v81";
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if !crate::hir::vec_constructor::program_uses(program)? {
        return super::string_slots::graph_schema(program);
    }
    crate::hir::validate(program)?;
    crate::loan_plan::validate_program(program)?;
    crate::hir::analyze_byte_data_capacity(program)?;
    Ok(SCHEMA)
}
pub(crate) fn legacy_graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if crate::hir::vec_constructor::program_uses(program)? {
        return Err(Diagnostic::io(
            "SPX-G410",
            "Vec constructors require Graph v81",
        ));
    }
    super::string_slots::legacy_graph_schema(program)
}
pub(super) fn graph_json(
    program: &ResolvedProgram,
    revision: &str,
    functions: &BTreeSet<DeclarationId>,
    types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    let mut graph = super::string_slots::graph_json(program, revision, functions, types, view)?;
    if !crate::hir::vec_constructor::program_uses(program)? {
        return Ok(graph);
    }
    graph_schema(program)?;
    let inner = super::string_slots::graph_schema(program)?;
    let prefix = crate::bounded_output::budgeted_format(format_args!("{{\"schema\":\"{inner}\""));
    if !graph.starts_with(&prefix) {
        return Err(Diagnostic::io(
            "SPX-G410",
            "noncanonical Vec-constructor graph",
        ));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v81\"");
    Ok(graph)
}
