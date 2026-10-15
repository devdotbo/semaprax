//! Complete original constructor graph receiver, independently compiled.
use super::super::*;

pub(in crate::hir::validation) fn validate_call_graph(
    program: &ResolvedProgram,
) -> Result<(), Diagnostic> {
    let ids = program
        .function_templates
        .iter()
        .map(|template| template.id.clone())
        .collect::<BTreeSet<_>>();
    let mut edges = BTreeMap::<DeclarationId, BTreeSet<DeclarationId>>::new();
    let mut incoming = ids
        .iter()
        .cloned()
        .map(|id| (id, 0usize))
        .collect::<BTreeMap<_, _>>();
    for template in &program.function_templates {
        for expression in template
            .requires
            .iter()
            .chain(std::iter::once(&template.body))
            .chain(&template.ensures)
        {
            visit_resolved_calls(expression, &mut |callee, _, _| {
                if ids.contains(callee)
                    && edges
                        .entry(template.id.clone())
                        .or_default()
                        .insert(callee.clone())
                {
                    *incoming.get_mut(callee).expect("template was indexed") += 1;
                }
            });
        }
    }
    let mut pending = incoming
        .iter()
        .filter_map(|(id, count)| (*count == 0).then_some(id.clone()))
        .collect::<std::collections::VecDeque<_>>();
    let mut visited = 0usize;
    while let Some(id) = pending.pop_front() {
        visited += 1;
        for callee in edges.get(&id).into_iter().flatten() {
            let count = incoming.get_mut(callee).expect("template was indexed");
            *count -= 1;
            if *count == 0 {
                pending.push_back(callee.clone());
            }
        }
    }
    (visited == ids.len())
        .then_some(())
        .ok_or_else(|| hir_error("generic function template call graph contains a direct cycle"))
}
