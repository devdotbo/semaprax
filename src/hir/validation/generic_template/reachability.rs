//! Borrowed bounded closure of materialized generic function instances.
//!
//! The pending queue, seen set and result borrow the exact instance IDs,
//! declaration IDs and complete argument carriers published by the program's
//! call expressions. Derive identity authentication still builds its own
//! canonical ID for every visited entry; that allocation is legitimate.
use super::*;

type Pending<'p> = std::collections::VecDeque<(
    &'p FunctionInstanceId,
    &'p DeclarationId,
    &'p [ResolvedType],
)>;

pub(in crate::hir::validation) type Reachable<'p> = Vec<(
    &'p FunctionInstanceId,
    &'p DeclarationId,
    &'p [ResolvedType],
)>;

pub(in crate::hir::validation) fn reachable_instances<'p>(
    program: &'p ResolvedProgram,
) -> Result<Reachable<'p>, Diagnostic> {
    const MAX_FUNCTION_INSTANCES: usize = 256;
    let mut seen = BTreeSet::new();
    let mut reachable = Vec::new();
    let mut pending = Pending::new();
    for function in &program.functions {
        collect_calls(function, &mut pending);
    }
    while let Some((instance, template, arguments)) = pending.pop_front() {
        if FunctionInstanceId::derive(template, arguments) != *instance {
            return Err(hir_error(
                "reachable generic function instance identity is inconsistent",
            ));
        }
        if !seen.insert(instance) {
            continue;
        }
        if reachable.len() == MAX_FUNCTION_INSTANCES {
            return Err(hir_error(format!(
                "generic function instance closure exceeds {MAX_FUNCTION_INSTANCES} entries"
            )));
        }
        reachable.push((instance, template, arguments));
        if let Some(materialized) = program
            .function_instances
            .iter()
            .find(|candidate| candidate.id == *instance)
        {
            collect_calls(&materialized.function, &mut pending);
        }
    }
    Ok(reachable)
}

fn collect_calls<'p>(function: &'p ResolvedFunction, pending: &mut Pending<'p>) {
    for expression in function
        .requires
        .iter()
        .chain(std::iter::once(&function.body))
        .chain(&function.ensures)
    {
        visit_resolved_calls(expression, &mut |callee, instance, arguments| {
            if let Some(instance) = instance {
                pending.push_back((instance, callee, arguments));
            }
        });
    }
}

#[cfg(test)]
mod tests;
