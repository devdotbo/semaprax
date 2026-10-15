//! Require each independently authenticated compiler-generated row type site
//! exactly once in retained HIR. The authored projection emits the same site
//! as a `type_reference` edge (`expected_projection::record_traversal`), so
//! the site stays in the exact multiset compared against those edges.
use super::*;

fn require_one(
    owner: &str,
    call_path: &str,
    target: &str,
    sites: &[type_projection::TypeSite<'_>],
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
    let mut found = false;
    for site in sites {
        if site.projection() == expected && std::mem::replace(&mut found, true) {
            return Err(vec![graph_error(
                "SPX-G173",
                "generated traversal type-site multiplicity disagrees",
            )]);
        }
    }
    if !found {
        return Err(vec![graph_error(
            "SPX-G173",
            "generated traversal type site is missing",
        )]);
    }
    Ok(())
}

// Call only after record_traversal::validate has independently proved all
// current HIR source-field/root/index/loop identities. Nothing is erased: a
// raw marker/name never authorizes dropping a type reference from the exact
// HIR/edge comparison.
pub(super) fn require(
    source: &Program,
    programs: &[Program],
    imported: &BTreeSet<&str>,
    sites: &[type_projection::TypeSite<'_>],
) -> Result<(), Vec<Diagnostic>> {
    let expected = super::super::record_traversal::collect(source, programs)?;
    for item in &expected.loops {
        if imported.contains(item.declaration.stable_id.as_str()) {
            let path =
                crate::bounded_output::budgeted_format(format_args!("{}.s0.value", item.path));
            require_one(
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
            require_one(
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
        let sites = fresh();
        require_one("app.main", call, "rows.Row", &sites).unwrap();
        // The generated site stays in the exact multiset compared to edges.
        assert_eq!(sites.len(), 3);
        let generated = ("app.main", expression.as_str(), path.as_str(), "rows.Row");
        assert_eq!(
            sites.iter().filter(|s| s.projection() == generated).count(),
            1
        );
        for (caller, current_call, target) in [
            ("foreign.main", call, "rows.Row"),
            ("app.main", "body.s1.value.s0.value", "rows.Row"),
            ("app.main", call, "foreign.Row"),
        ] {
            let sites = fresh();
            assert_eq!(
                require_one(caller, current_call, target, &sites).unwrap_err()[0].code,
                "SPX-G173"
            );
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
            require_one("app.main", call, "rows.Row", &sites).unwrap_err()[0].code,
            "SPX-G173"
        );
        assert_eq!(sites.len(), 4);
    }
}
