//! Remove only independently authenticated compiler-generated row type sites.
//! Source declarations/calls retain their explicit reference multiplicity.
use super::*;

fn remove_one(
    owner: &str,
    call_path: &str,
    target: &str,
    sites: &mut Vec<type_projection::TypeSite<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    let owner_id = hir::DeclarationId::new(crate::bounded_output::budgeted_clone(owner));
    let expression = hir::budgeted_workspace_expression_identity(&owner_id, call_path);
    let path = crate::bounded_output::budgeted_format(format_args!("{call_path}.type_argument.0"));
    if crate::bounded_output::active_refused() {
        return Err(vec![limit_error(
            "builder_bytes",
            super::super::active_builder_limit(),
        )]);
    }
    let expected = (owner, expression.as_str(), path.as_str(), target);
    let mut found = None;
    for (index, site) in sites.iter().enumerate() {
        if site.projection() == expected && found.replace(index).is_some() {
            return Err(vec![graph_error(
                "SPX-G173",
                "generated traversal type-site multiplicity disagrees",
            )]);
        }
    }
    let Some(index) = found else {
        return Err(vec![graph_error(
            "SPX-G173",
            "generated traversal type site is missing",
        )]);
    };
    sites.remove(index);
    Ok(())
}

// Call only after record_traversal::validate has independently proved all
// current HIR source-field/root/index/loop identities. A raw marker/name does
// not authorize erasing a type reference.
pub(super) fn remove(
    source: &Program,
    programs: &[Program],
    imported: &BTreeSet<&str>,
    sites: &mut Vec<type_projection::TypeSite<'_>>,
) -> Result<(), Vec<Diagnostic>> {
    let expected = super::super::record_traversal::collect(source, programs)?;
    for item in &expected.loops {
        if imported.contains(item.declaration.stable_id.as_str()) {
            let path =
                crate::bounded_output::budgeted_format(format_args!("{}.s0.value", item.path));
            remove_one(
                &item.function.stable_id,
                &path,
                &item.declaration.stable_id,
                sites,
            )?;
        }
    }
    for read in &expected.reads {
        if imported.contains(read.declaration.stable_id.as_str()) {
            let path = if read.copy {
                crate::bounded_output::budgeted_format(format_args!("{}.base", read.path))
            } else if read.bytes {
                crate::bounded_output::budgeted_format(format_args!("{}.arg.0", read.path))
            } else {
                crate::bounded_output::budgeted_clone(&read.path)
            };
            remove_one(
                &read.function.stable_id,
                &path,
                &read.declaration.stable_id,
                sites,
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn generated_type_tuple_is_singleton_and_authored_sites_keep_exact_multiplicity() {
        let owner = hir::DeclarationId::new("app.main");
        let call = "body.s0.value.s0.value";
        let expression = hir::workspace_expression_identity(&owner, call);
        let path = format!("{call}.type_argument.0");
        let authored = hir::workspace_expression_identity(&owner, "body.tail");
        let fresh = || {
            let mut sites = Vec::new();
            type_projection::TypeSite::push(
                &mut sites,
                "app.main",
                Some(expression.as_str()),
                &path,
                "rows.Row",
            )
            .unwrap();
            // Repeated authored type arguments remain repeated.
            for _ in 0..2 {
                type_projection::TypeSite::push(
                    &mut sites,
                    "app.main",
                    Some(authored.as_str()),
                    "body.tail.type_argument.0",
                    "rows.Row",
                )
                .unwrap();
            }
            sites
        };
        let mut sites = fresh();
        remove_one("app.main", call, "rows.Row", &mut sites).unwrap();
        assert_eq!(sites.len(), 2);
        assert!(sites.iter().all(|s| s.projection()
            == (
                "app.main",
                authored.as_str(),
                "body.tail.type_argument.0",
                "rows.Row"
            )));
        for (caller, current_call, target) in [
            ("foreign.main", call, "rows.Row"),
            ("app.main", "body.s1.value.s0.value", "rows.Row"),
            ("app.main", call, "foreign.Row"),
        ] {
            let mut sites = fresh();
            assert_eq!(
                remove_one(caller, current_call, target, &mut sites).unwrap_err()[0].code,
                "SPX-G173"
            );
            assert_eq!(sites.len(), 3);
        }
        let mut sites = fresh();
        type_projection::TypeSite::push(
            &mut sites,
            "app.main",
            Some(expression.as_str()),
            &path,
            "rows.Row",
        )
        .unwrap();
        assert_eq!(
            remove_one("app.main", call, "rows.Row", &mut sites).unwrap_err()[0].code,
            "SPX-G173"
        );
        assert_eq!(sites.len(), 4);
    }
}
