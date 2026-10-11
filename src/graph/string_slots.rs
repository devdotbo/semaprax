//! Selective v80 String-slot views retain every inner v79/v78/v77 fact.
use super::{BTreeSet, DeclarationId, Diagnostic, GraphView, ResolvedProgram};
const SCHEMA: &str = "semaprax.graph.v80";
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if !crate::vec_ops::string_slot::resolved_uses(program) {
        return super::consuming_vec_renewal::graph_schema(program);
    }
    crate::hir::validate(program)?;
    crate::loan_plan::validate_program(program)?;
    crate::hir::analyze_byte_data_capacity(program)?;
    Ok(SCHEMA)
}
pub(crate) fn legacy_graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if crate::vec_ops::string_slot::resolved_uses(program) {
        return Err(Diagnostic::io(
            "SPX-G410",
            "String slot views require Graph v80",
        ));
    }
    super::consuming_vec_renewal::legacy_graph_schema(program)
}
pub(super) fn graph_json(
    program: &ResolvedProgram,
    revision: &str,
    functions: &BTreeSet<DeclarationId>,
    types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    let mut graph =
        super::consuming_vec_renewal::graph_json(program, revision, functions, types, view)?;
    if !crate::vec_ops::string_slot::resolved_uses(program) {
        return Ok(graph);
    }
    graph_schema(program)?;
    let inner = super::consuming_vec_renewal::graph_schema(program)?;
    let prefix = crate::bounded_output::budgeted_format(format_args!("{{\"schema\":\"{inner}\""));
    if !graph.starts_with(&prefix) {
        return Err(Diagnostic::io("SPX-G410", "noncanonical String-slot graph"));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v80\"");
    Ok(graph)
}

pub(super) fn root_kind_json(kind: crate::hir::ByteSliceRootKind) -> &'static str {
    use crate::hir::ByteSliceRootKind;
    match kind {
        ByteSliceRootKind::FunctionParameter => "function_parameter",
        ByteSliceRootKind::SourceExprStatic => "source_expr_static",
        ByteSliceRootKind::OwnedBytes => "owned_bytes",
        ByteSliceRootKind::OwnedString => "owned_string",
        ByteSliceRootKind::OwnedVectorField => "owned_vector_field",
        ByteSliceRootKind::OwnedVectorStringSlot => "owned_vector_string_slot",
        ByteSliceRootKind::FixedArray => "fixed_array",
        ByteSliceRootKind::BorrowedStr => "borrowed_str",
        ByteSliceRootKind::CommandArguments => "command_arguments",
        ByteSliceRootKind::StdinStreamReader => "stdin_stream_reader",
    }
}
