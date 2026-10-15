use super::{
    visit_expr_calls, visit_function_calls, work_counter, BTreeSet, DeclarationId, ResolvedFunction,
};

pub(super) fn function_calls(function: &ResolvedFunction) -> BTreeSet<DeclarationId> {
    work_counter::record(work_counter::Work::CallSetComputation, 1);
    let mut calls = BTreeSet::new();
    visit_function_calls(function, &mut |callee| {
        calls.insert(callee.clone());
    });
    calls
}

pub(super) fn template_calls(
    template: &crate::hir::ResolvedFunctionTemplate,
) -> BTreeSet<DeclarationId> {
    work_counter::record(work_counter::Work::CallSetComputation, 1);
    let mut calls = BTreeSet::new();
    for expression in template
        .requires
        .iter()
        .chain(std::iter::once(&template.body))
        .chain(&template.ensures)
    {
        visit_expr_calls(expression, &mut |callee| {
            calls.insert(callee.clone());
        });
    }
    calls
}

use super::{quote_json, type_json, visit_expr_call_instances};
use crate::bounded_output::BudgetedJoin as _;
pub(super) fn agent_call_instances_json(function: &ResolvedFunction) -> String {
    let mut calls = Vec::new();
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        visit_expr_call_instances(
            expression,
            &mut |expression, callee, type_arguments, instance| {
                calls.push(format!(
                    "{{\"expression\":{},\"template\":{},\"instance\":{},\"type_arguments\":[{}]}}",
                    quote_json(expression.id.as_str()),
                    quote_json(callee.as_str()),
                    quote_json(instance.as_str()),
                    type_arguments
                        .iter()
                        .map(type_json)
                        .collect::<Vec<_>>()
                        .budgeted_join(",")
                ));
            },
        );
    }
    calls.budgeted_join(",")
}
