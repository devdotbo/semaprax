//! Selective v83 borrowed `Slice<u8>` traversals retain every inner v81/v80/v79/v78/v77 fact.
//!
//! Graph v83 owns the traversal's additive spellings: the single-arm receiver
//! match projects `"exhaustive":false`, a lexical latch projects
//! `"source_kind":"borrow_bytes"`, and the owner freeze is an ordinary
//! `traversal_freeze` loan. A program without a receiver keeps its exact inner
//! schema and bytes.
use super::{BTreeSet, DeclarationId, Diagnostic, GraphView, ResolvedProgram};
const SCHEMA: &str = "semaprax.graph.v83";
pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if !crate::hir::byte_traversal_program_uses(program) {
        return super::row_loan::graph_schema(program);
    }
    crate::hir::validate(program)?;
    crate::loan_plan::validate_program(program)?;
    crate::hir::analyze_byte_data_capacity(program)?;
    Ok(SCHEMA)
}
pub(crate) fn legacy_graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if crate::hir::byte_traversal_program_uses(program) {
        return Err(Diagnostic::io(
            "SPX-G410",
            "borrowed Slice<u8> traversal requires Graph v83",
        ));
    }
    super::row_loan::legacy_graph_schema(program)
}
pub(super) fn graph_json(
    program: &ResolvedProgram,
    revision: &str,
    functions: &BTreeSet<DeclarationId>,
    types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    let mut graph = super::row_loan::graph_json(program, revision, functions, types, view)?;
    if !crate::hir::byte_traversal_program_uses(program) {
        return Ok(graph);
    }
    graph_schema(program)?;
    let inner = super::row_loan::graph_schema(program)?;
    let prefix = crate::bounded_output::budgeted_format(format_args!("{{\"schema\":\"{inner}\""));
    if !graph.starts_with(&prefix) {
        return Err(Diagnostic::io(
            "SPX-G410",
            "noncanonical borrowed byte traversal graph",
        ));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v83\"");
    Ok(graph)
}
