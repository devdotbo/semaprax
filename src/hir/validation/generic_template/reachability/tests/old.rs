//! Complete original owned reachable-instance closure, retained as an independent reference.
use super::super::*;

pub(super) fn reachable_instances(
    program: &ResolvedProgram,
) -> Result<Vec<(FunctionInstanceId, DeclarationId, Vec<ResolvedType>)>, Diagnostic> {
    const MAX_FUNCTION_INSTANCES: usize = 256;
    let mut seen = BTreeSet::new();
    let mut reachable = Vec::new();
    let mut pending = std::collections::VecDeque::new();
    for function in &program.functions {
        collect_calls(function, &mut pending);
    }
    while let Some((instance, template, arguments)) = pending.pop_front() {
        if FunctionInstanceId::derive(&template, &arguments) != instance {
            return Err(hir_error(
                "reachable generic function instance identity is inconsistent",
            ));
        }
        if !seen.insert(instance.clone()) {
            continue;
        }
        if reachable.len() == MAX_FUNCTION_INSTANCES {
            return Err(hir_error(format!(
                "generic function instance closure exceeds {MAX_FUNCTION_INSTANCES} entries"
            )));
        }
        reachable.push((instance.clone(), template, arguments));
        if let Some(materialized) = program
            .function_instances
            .iter()
            .find(|candidate| candidate.id == instance)
        {
            collect_calls(&materialized.function, &mut pending);
        }
    }
    Ok(reachable)
}

fn collect_calls(
    function: &ResolvedFunction,
    pending: &mut std::collections::VecDeque<(
        FunctionInstanceId,
        DeclarationId,
        Vec<ResolvedType>,
    )>,
) {
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        visit_resolved_calls(expression, &mut |callee, instance, arguments| {
            if let Some(instance) = instance {
                pending.push_back((instance.clone(), callee.clone(), arguments.to_vec()));
            }
        });
    }
}
