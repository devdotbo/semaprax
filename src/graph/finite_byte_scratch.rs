//! Selective Graph v78: independently recovered finite iterator resource bounds.
use super::*;
use crate::bounded_output::BudgetedJoin as _;
const SCHEMA: &str = "semaprax.graph.v78";
pub(super) fn requires(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(crate::finite_byte_scratch::function_requires)
}
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if !requires(program) {
        return super::static_text::graph_schema(program);
    }
    hir::validate(program)?;
    crate::loan_plan::validate_program(program)?;
    hir::analyze_byte_data_capacity(program)?;
    Ok(SCHEMA)
}
pub(crate) fn legacy_graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if requires(program) {
        return Err(Diagnostic::io(
            "SPX-G410",
            "finite owned-iterator scratch requires Graph v78",
        ));
    }
    super::static_text::legacy_graph_schema(program)
}
pub(super) fn graph_json(
    program: &ResolvedProgram,
    revision: &str,
    functions: &BTreeSet<DeclarationId>,
    types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    let mut graph = super::static_text::graph_json(program, revision, functions, types, view)?;
    if !requires(program) {
        return Ok(graph);
    }
    graph_schema(program)?;
    let document: serde_json::Value = serde_json::from_str(&graph)
        .map_err(|_| Diagnostic::io("SPX-G410", "invalid checked finite iterator graph"))?;
    let previous = document["schema"]
        .as_str()
        .ok_or_else(|| Diagnostic::io("SPX-G410", "missing checked schema"))?;
    let prefix = format!("{{\"schema\":{}", quote_json(previous));
    if !graph.starts_with(&prefix) || !graph.ends_with('}') {
        return Err(Diagnostic::io(
            "SPX-G410",
            "noncanonical finite iterator graph",
        ));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v78\"");
    let mut loops = Vec::new();
    for (identity, function) in program.functions.iter().map(|f| (f.id.as_str(), f)).chain(
        program
            .function_instances
            .iter()
            .map(|i| (i.id.as_str(), &i.function)),
    ) {
        if !functions.contains(&function.id) {
            continue;
        }
        for (condition, body) in crate::finite_byte_scratch::loops(function) {
            if let Some(proof) =
                crate::finite_byte_scratch::resolved_function(function, condition, body)
            {
                if !crate::bounded_output::reserve_active_required(
                    std::mem::size_of::<String>() * (1 + proof.sites().count()),
                ) {
                    return Err(Diagnostic::io(
                        "SPX-G171",
                        "finite scratch graph descriptor backing exceeds its builder budget",
                    ));
                }
                loops.reserve_exact(1);
                loops.push(format!("{{\"execution\":{},\"condition\":{},\"body\":{},\"iterations\":{},\"allocation_sites\":[{}]}}", quote_json(identity), quote_json(condition.id.as_str()), quote_json(body.id.as_str()), proof.iterations, proof.sites().map(|site| quote_json(site.id.as_str())).collect::<Vec<_>>().budgeted_join(",")));
            }
        }
    }
    graph.pop();
    graph.push_str(&format!(",\"finite_owned_iterator_scratch\":{{\"schema\":\"semaprax.finite-owned-iterator-scratch.v1\",\"max_allocation_events\":32,\"max_payload_bytes\":2097152,\"refund_on_drop\":false,\"loops\":[{}]}}}}", loops.budgeted_join(",")));
    Ok(graph)
}
