//! Selective v79 retains finite/static facts and rederived consuming-row renewal.
use super::*;
const SCHEMA: &str = "semaprax.graph.v79";
pub(super) fn requires(program: &ResolvedProgram) -> bool {
    program
        .functions
        .iter()
        .chain(program.function_instances.iter().map(|i| &i.function))
        .any(|function| hir::vec_loop_renewal::consuming::requires(program, function))
}
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if !requires(program) {
        return super::finite_byte_scratch::graph_schema(program);
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
            "consuming owned-record renewal requires Graph v79",
        ));
    }
    super::finite_byte_scratch::legacy_graph_schema(program)
}
pub(super) fn graph_json(
    program: &ResolvedProgram,
    revision: &str,
    functions: &BTreeSet<DeclarationId>,
    types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    // The inner provider keeps every selected v78/v77/older fact.
    let mut graph =
        super::finite_byte_scratch::graph_json(program, revision, functions, types, view)?;
    if !requires(program) {
        return Ok(graph);
    }
    graph_schema(program)?;
    // Only inspect the provider's canonical leading schema token. Building a
    // serde DOM here would duplicate the complete graph outside its census.
    const HEADER: &str = "{\"schema\":\"";
    let token = graph.strip_prefix(HEADER).and_then(|tail| {
        let end = tail
            .as_bytes()
            .iter()
            .take(64)
            .position(|byte| *byte == b'"')?;
        let schema = &tail[..end];
        let version = schema.strip_prefix("semaprax.graph.v")?;
        (!version.is_empty() && version.bytes().all(|byte| byte.is_ascii_digit()))
            .then_some(HEADER.len() + end + 1)
    });
    let prefix_len = token
        .filter(|_| graph.ends_with('}'))
        .ok_or_else(|| Diagnostic::io("SPX-G410", "noncanonical consuming renewal graph"))?;
    graph.replace_range(..prefix_len, "{\"schema\":\"semaprax.graph.v79\"");
    let mut updates = Vec::new();
    for (execution, function) in program.functions.iter().map(|f| (f.id.as_str(), f)).chain(
        program
            .function_instances
            .iter()
            .map(|i| (i.id.as_str(), &i.function)),
    ) {
        if !functions.contains(&function.id) {
            continue;
        }
        for (_, site) in hir::vec_loop_renewal::consuming::sites(program, function) {
            if !crate::bounded_output::reserve_active_required(std::mem::size_of::<String>()) {
                return Err(Diagnostic::io(
                    "SPX-G171",
                    "consuming renewal graph backing exceeds its builder budget",
                ));
            }
            updates.reserve_exact(1);
            updates.push(format!("{{\"execution\":{},\"condition\":{},\"body\":{},\"item\":{},\"at\":{},\"binding\":{},\"element\":{}}}",
                quote_json(execution), quote_json(site.condition.id.as_str()), quote_json(site.body.id.as_str()),
                quote_json(site.item.id.as_str()), quote_json(site.at.id.as_str()), quote_json(site.binding.id.as_str()), type_json(&site.item.ty)));
        }
    }
    graph.pop();
    graph.push_str(&format!(",\"consuming_owned_record_renewal\":{{\"schema\":\"semaprax.consuming-owned-record-renewal.v1\",\"updates\":[{}]}}}}", updates.budgeted_join(",")));
    Ok(graph)
}
