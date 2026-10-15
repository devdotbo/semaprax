//! Saved owned discovery implementation; independent output/error oracle.
use super::*;
pub(super) fn link(
    workspace: &WorkspaceGraphBuild,
    root_module: &str,
    export_ids: &[String],
) -> Result<hir::ResolvedProgram, Vec<Diagnostic>> {
    validate_entry_module(root_module)?;
    if export_ids.is_empty() {
        return Err(vec![graph_error(
            "SPX-PS504",
            "package-source root export inventory is empty",
        )]);
    }
    if export_ids
        .windows(2)
        .any(|pair| pair[0].as_bytes() >= pair[1].as_bytes())
    {
        return Err(vec![graph_error(
            "SPX-PS503",
            "package-source root exports are not strictly byte-sorted and unique",
        )]);
    }

    let mut available = BTreeMap::<hir::DeclarationId, hir::LinkedScalarFunction>::new();
    for module in &workspace.hir.modules {
        for function in &module.functions {
            let Some(fact) = workspace.hir.declarations.get(function.id.as_str()) else {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "package-source function is absent from declaration facts",
                )]);
            };
            if fact.kind != hir::DeclarationKind::Function
                || fact.path.as_deref() != Some(module.path.as_str())
                || fact.module.as_deref() != Some(module.module.as_str())
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "package-source function declaration facts disagree with retained body",
                )]);
            }
            if available
                .insert(
                    function.id.clone(),
                    hir::LinkedScalarFunction {
                        function: function.clone(),
                        origin: fact.origin,
                    },
                )
                .is_some()
            {
                return Err(vec![graph_error(
                    "SPX-G173",
                    "package-source function identity is duplicated",
                )]);
            }
        }
    }

    let mut anchor = None;
    let mut pending = BTreeSet::new();
    for export in export_ids {
        let id = hir::DeclarationId::new(export.clone());
        let Some(linked) = available.get(&id) else {
            return Err(vec![graph_error(
                "SPX-PS503",
                "package-source root export is absent from retained HIR",
            )]);
        };
        let fact = workspace
            .hir
            .declarations
            .get(linked.function.id.as_str())
            .expect("available functions have declaration facts");
        if fact.module.as_deref() != Some(root_module)
            || fact.origin != hir::IdentityOrigin::Explicit
        {
            return Err(vec![graph_error(
                "SPX-PS503",
                "package-source export is not an explicit root-owned function",
            )]);
        }
        if anchor.is_none()
            && linked.function.params.is_empty()
            && linked.function.return_type == hir::ResolvedType::I64
        {
            anchor = Some(id.clone());
        }
        pending.insert(id);
    }
    let anchor = anchor.ok_or_else(|| {
        vec![graph_error(
            "SPX-PS504",
            "package-source exports require a byte-lowest fn() -> i64 HIR anchor",
        )]
    })?;

    let mut retained = BTreeSet::new();
    while let Some(function_id) = pending.pop_first() {
        let linked = available.get(&function_id).ok_or_else(|| {
            vec![graph_error(
                "SPX-G173",
                "package-source call closure names an unauthenticated function",
            )]
        })?;
        if !retained.insert(function_id) {
            continue;
        }
        for callee in resolved_function_callees(&linked.function) {
            if available.contains_key(&callee) && !retained.contains(&callee) {
                pending.insert(callee);
            }
        }
    }
    let functions = retained
        .into_iter()
        .map(|id| {
            available
                .get(&id)
                .map(|linked| hir::LinkedScalarFunction {
                    function: linked.function.clone(),
                    origin: linked.origin,
                })
                .ok_or_else(|| {
                    vec![graph_error(
                        "SPX-G173",
                        "package-source retained closure lost an authenticated function",
                    )]
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    hir::link_package_scalar_workspace(root_module.to_owned(), anchor, functions)
        .map_err(|error| vec![error])
}
