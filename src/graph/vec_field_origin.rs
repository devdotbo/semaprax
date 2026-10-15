//! Selective Graph v85 retains explicit vs row-derived checked read semantics.
use super::{BTreeSet, DeclarationId, Diagnostic, GraphView, ResolvedProgram};
pub(crate) const SCHEMA: &str = "semaprax.graph.v85";
fn requires(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(|f| {
            std::iter::once(&f.body)
                .chain(&f.requires)
                .chain(&f.ensures)
                .any(crate::vec_field::expression_uses)
        })
}
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    let inner = super::copy_discard::graph_schema(program)?;
    if requires(program) {
        crate::hir::validate(program)?;
        crate::loan_plan::validate_program(program)?;
        Ok(SCHEMA)
    } else {
        Ok(inner)
    }
}
pub(crate) fn legacy_graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if requires(program) {
        return Err(Diagnostic::io(
            "SPX-G410",
            "scoped Vec field read source forms require Graph v85",
        ));
    }
    super::copy_discard::legacy_graph_schema(program)
}
pub(super) fn graph_json(
    program: &ResolvedProgram,
    revision: &str,
    functions: &BTreeSet<DeclarationId>,
    types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    let selected = requires(program);
    if selected {
        graph_schema(program)?;
    }
    let mut graph = super::copy_discard::graph_json(program, revision, functions, types, view)?;
    if !selected {
        return Ok(graph);
    }
    let inner = super::copy_discard::graph_schema(program)?;
    let prefix = crate::bounded_output::budgeted_format(format_args!("{{\"schema\":\"{inner}\""));
    if !graph.starts_with(&prefix) {
        return Err(Diagnostic::io(
            "SPX-G410",
            "noncanonical scoped Vec field source-form graph",
        ));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v85\"");
    Ok(graph)
}

#[cfg(test)]
mod tests {
    #[test]
    fn source_form_schema_preserves_inner_facts_and_stays_outside_evidence() {
        assert!(super::super::graph_schema_includes_loans(super::SCHEMA));
        assert!(super::super::graph_schema_includes_modern_composite_facts(
            super::SCHEMA
        ));
        assert!(super::super::graph_schema_includes_projected_provenance(
            super::SCHEMA
        ));
        assert!(super::super::reject_evidence_schema(super::SCHEMA).is_err());
        assert!(!super::super::graph_schema_includes_loans(
            "semaprax.graph.v85 "
        ));
    }
}
