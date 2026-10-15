//! Source-bound transitive byte-helper renewal; older graph projections stay exact.
use super::*;
const SCHEMA: &str = "semaprax.graph.v76";
pub(super) fn requires(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(|f| !crate::loop_calls::byte_writer::bindings(program, f).is_empty())
}
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if !requires(program) {
        return super::scoped_vec_field::graph_schema(program);
    }
    hir::validate(program)?;
    crate::loan_plan::validate_program(program)?;
    Ok(SCHEMA)
}
pub(super) fn graph_json(
    program: &ResolvedProgram,
    revision: &str,
    functions: &BTreeSet<DeclarationId>,
    types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    let mut graph = super::scoped_vec_field::graph_json(program, revision, functions, types, view)?;
    if !requires(program) {
        return Ok(graph);
    }
    graph_schema(program)?;
    let document: serde_json::Value = serde_json::from_str(&graph)
        .map_err(|_| Diagnostic::io("SPX-G411", "invalid checked graph payload"))?;
    let previous = document["schema"]
        .as_str()
        .ok_or_else(|| Diagnostic::io("SPX-G411", "missing checked schema"))?;
    let prefix = format!("{{\"schema\":{}", quote_json(previous));
    if !graph.starts_with(&prefix) || !graph.ends_with('}') {
        return Err(Diagnostic::io("SPX-G411", "noncanonical checked graph"));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v76\"");
    let mut updates = Vec::new();
    for function in program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .filter(|f| functions.contains(&f.id))
    {
        for (at, binding) in crate::loop_calls::byte_writer::bindings(program, function) {
            updates.push(format!(
                "{{\"function\":{},\"at\":{},\"binding\":{}}}",
                quote_json(function.id.as_str()),
                quote_json(at.as_str()),
                quote_json(binding.id.as_str())
            ));
        }
    }
    graph.pop();
    graph.push_str(&format!(",\"byte_helper_renewal\":{{\"schema\":\"semaprax.byte-helper-renewal.v1\",\"updates\":[{}]}}}}", updates.join(",")));
    Ok(graph)
}
