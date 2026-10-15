//! Graph v82 is selected only by Row Loan v1 call arguments. It retains every
//! inner v81 fact unchanged and projects each descriptor as a `row_loan` node;
//! programs without row loans keep their exact older schema and bytes.
use super::{BTreeSet, DeclarationId, Diagnostic, GraphView, ResolvedProgram};
pub(crate) const SCHEMA: &str = "semaprax.graph.v82";

/// Whether any function body or contract of `functions` lends a row.
pub(crate) fn functions_require<'a>(
    functions: impl IntoIterator<Item = &'a crate::hir::ResolvedFunction>,
) -> bool {
    functions.into_iter().any(|function| {
        std::iter::once(&function.body)
            .chain(&function.requires)
            .chain(&function.ensures)
            .any(expression_uses)
    })
}

fn expression_uses(root: &crate::hir::ResolvedExpr) -> bool {
    let mut pending = vec![root];
    while let Some(expression) = pending.pop() {
        if matches!(
            expression.kind,
            crate::hir::ResolvedExprKind::RowLoan { .. }
        ) {
            return true;
        }
        if let crate::hir::ResolvedExprKind::Closure { body, .. } = &expression.kind {
            pending.push(body);
        }
        crate::hir::push_resolved_expression_children_in_authored_order(expression, &mut pending);
    }
    false
}

pub(crate) fn requires(program: &ResolvedProgram) -> bool {
    functions_require(
        program
            .functions
            .iter()
            .chain(program.function_instances.iter().map(|i| &i.function)),
    )
}

pub(crate) fn graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if !requires(program) {
        return super::vec_constructor::graph_schema(program);
    }
    crate::hir::validate(program)?;
    crate::loan_plan::validate_program(program)?;
    Ok(SCHEMA)
}

pub(crate) fn legacy_graph_schema(program: &ResolvedProgram) -> Result<&'static str, Diagnostic> {
    if requires(program) {
        return Err(Diagnostic::io(
            "SPX-G410",
            "record row loans require Graph v82",
        ));
    }
    super::vec_constructor::legacy_graph_schema(program)
}

pub(super) fn graph_json(
    program: &ResolvedProgram,
    revision: &str,
    functions: &BTreeSet<DeclarationId>,
    types: &BTreeSet<DeclarationId>,
    view: &GraphView<'_>,
) -> Result<String, Diagnostic> {
    let mut graph = super::vec_constructor::graph_json(program, revision, functions, types, view)?;
    if !requires(program) {
        return Ok(graph);
    }
    graph_schema(program)?;
    let inner = super::vec_constructor::graph_schema(program)?;
    let prefix = crate::bounded_output::budgeted_format(format_args!("{{\"schema\":\"{inner}\""));
    if !graph.starts_with(&prefix) || !graph.ends_with('}') {
        return Err(Diagnostic::io("SPX-G410", "noncanonical row-loan graph"));
    }
    graph.replace_range(..prefix.len(), "{\"schema\":\"semaprax.graph.v82\"");
    graph.pop();
    graph.push_str(",\"row_loans\":{\"schema\":\"semaprax.row-loan.v1\",\"authority\":false}}");
    Ok(graph)
}

/// One descriptor node. Its children are the exact Vec and index places.
pub(super) fn expression_json(
    program: &ResolvedProgram,
    header: &str,
    loan: &crate::hir::RowLoan,
    args: &[crate::hir::ResolvedExpr],
) -> Result<String, Diagnostic> {
    use crate::bounded_output::BudgetedJoin as _;
    let arguments = args
        .iter()
        .map(|argument| super::expr_json(program, argument))
        .collect::<Result<Vec<_>, _>>()?
        .budgeted_join(",");
    Ok(crate::bounded_output::budgeted_format(format_args!(
        "{{{header},\"kind\":\"row_loan\",\"version\":{},\"element_type\":{},\"traversal\":{},\"bound\":{},\"callee\":{},\"parameter\":{},\"args\":[{arguments}]}}",
        crate::diagnostic::quote_json(loan.version.as_str()),
        super::type_json(&loan.element),
        crate::diagnostic::quote_json(loan.traversal.as_str()),
        crate::diagnostic::quote_json(loan.bound.as_str()),
        crate::diagnostic::quote_json(loan.callee.as_str()),
        crate::diagnostic::quote_json(loan.parameter.as_str()),
    )))
}
