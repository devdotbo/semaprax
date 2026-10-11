use super::super::{signature_owner, LayoutClosure};
use crate::workspace_graph::index_authored;
const PROVIDER: &str = include_str!("../function_provider.spx");
const CALLER: &str = include_str!("../function_caller.spx");
fn sources() -> [crate::ast::Program; 2] {
    [
        crate::parse(CALLER, "caller.spx").unwrap(),
        crate::parse(PROVIDER, "provider.spx").unwrap(),
    ]
}
#[test]
fn held_signature_sites_match_independent_walk_without_per_query_backing() {
    let parsed = sources();
    let authored = index_authored(&parsed).unwrap();
    let layout = LayoutClosure::derive(&parsed[0], &authored, &parsed).unwrap();
    for function in &parsed[1].functions {
        let target = authored.get(function.stable_id.as_str()).unwrap();
        if !parsed[0]
            .module_uses
            .iter()
            .any(|u| u.persistent_id == function.stable_id)
        {
            continue;
        }
        for ty in
            std::iter::once(&function.return_type).chain(function.params.iter().map(|p| &p.ty))
        {
            let expected = signature_owner(&parsed[0], target.module, ty, &authored, &parsed);
            assert_eq!(
                layout
                    .signature_owner(&parsed[0], target, ty, &authored, &parsed)
                    .unwrap(),
                expected
            );
            let (answer, overflow, bytes) = crate::bounded_output::with_limit_usage(0, || {
                for _ in 0..32 {
                    assert_eq!(
                        layout.signature_owner(&parsed[0], target, ty, &authored, &parsed)?,
                        expected
                    );
                }
                Ok::<_, Vec<crate::diagnostic::Diagnostic>>(())
            });
            answer.unwrap();
            assert!(!overflow);
            assert_eq!(bytes, 0);
        }
    }
    // Many authentic sites sharing one nominal root use logarithmic directory
    // probes, rather than rebuilding or rescanning the signature prefix.
    let params = (0..64)
        .map(|n| format!("v{n}:borrow Report"))
        .collect::<Vec<_>>()
        .join(",");
    let provider = format!("{PROVIDER}\n@id(\"wide\") fn wide({params})->i64 {{0}}\n");
    let caller =
        format!("{CALLER}\nuse function @id(\"wide\") from function_layout.provider as wide;\n");
    let wide = [
        crate::parse(&caller, "caller.spx").unwrap(),
        crate::parse(&provider, "provider.spx").unwrap(),
    ];
    let wide_authored = index_authored(&wide).unwrap();
    let held = LayoutClosure::derive(&wide[0], &wide_authored, &wide).unwrap();
    let owners = held.owners.as_ref().unwrap();
    let target = wide_authored.get("wide").unwrap();
    let function = target.function.unwrap();
    let probes = usize::BITS as usize - owners.sites.len().leading_zeros() as usize;
    let max_id = owners
        .sites
        .iter()
        .map(|s| s.function.stable_id.len())
        .max()
        .unwrap();
    for parameter in &function.params {
        let before = owners.ledger.remaining.get();
        assert_eq!(
            held.signature_owner(&wide[0], target, &parameter.ty, &wide_authored, &wide)
                .unwrap(),
            Some(true)
        );
        let charged = before - owners.ledger.remaining.get();
        assert!(charged <= 1 + probes * (function.stable_id.len() + max_id + 1));
        assert_eq!(
            signature_owner(
                &wide[0],
                target.module,
                &parameter.ty,
                &wide_authored,
                &wide
            ),
            Some(true)
        );
    }
    let target = authored.get("make").unwrap();
    let ty = &target.function.unwrap().return_type;
    let cloned: crate::ast::Type = (*ty).clone();
    assert!(layout
        .signature_owner(&parsed[0], target, &cloned, &authored, &parsed)
        .is_err());
    let copy = parsed[0].clone();
    assert!(layout
        .signature_owner(&copy, target, ty, &authored, &parsed)
        .is_err());
    let stale = parsed.clone();
    let stale_authored = index_authored(&stale).unwrap();
    assert!(layout
        .signature_owner(
            &parsed[0],
            stale_authored.get("make").unwrap(),
            ty,
            &stale_authored,
            &stale
        )
        .is_err());
}
#[test]
fn held_signature_directory_has_exact_one_short_storage_and_shared_lookup_work() {
    let parsed = sources();
    let authored = index_authored(&parsed).unwrap();
    let derive = || LayoutClosure::derive(&parsed[0], &authored, &parsed);
    let (answer, overflow, bytes) = crate::bounded_output::with_limit_usage(usize::MAX, derive);
    answer.unwrap();
    assert!(!overflow);
    assert!(bytes > 0);
    let (answer, overflow, _) = crate::bounded_output::with_limit_usage(bytes, derive);
    answer.unwrap();
    assert!(!overflow);
    let (answer, overflow, _) = crate::bounded_output::with_limit_usage(bytes - 1, derive);
    assert!(answer.is_err());
    assert!(overflow);
    let layout = derive().unwrap();
    let owners = layout.owners.as_ref().unwrap();
    let target = authored.get("make").unwrap();
    let ty = &target.function.unwrap().return_type;
    let start = owners.ledger.remaining.get();
    layout
        .signature_owner(&parsed[0], target, ty, &authored, &parsed)
        .unwrap();
    let exact = start - owners.ledger.remaining.get();
    assert!(exact > 0);
    owners.ledger.remaining.set(exact);
    assert_eq!(
        layout
            .signature_owner(&parsed[0], target, ty, &authored, &parsed)
            .unwrap(),
        Some(true)
    );
    assert_eq!(owners.ledger.remaining.get(), 0);
    owners.ledger.remaining.set(exact - 1);
    assert!(layout
        .signature_owner(&parsed[0], target, ty, &authored, &parsed)
        .is_err());
    assert!(layout
        .signature_owner(&parsed[0], target, ty, &authored, &parsed)
        .is_err());
    assert!(owners.ledger.failed.get());
    assert_eq!(owners.ledger.remaining.get(), 0);
    let ((admitted, overflow, backing), legacy_scans) =
        crate::workspace_graph::owned_function_import::with_legacy_record_scans(|| {
            crate::bounded_output::with_limit_usage(0, || {
                crate::workspace_graph::owned_function_import::admitted_with_layout(
                    &parsed[0], target, &authored, &parsed, &layout,
                )
            })
        });
    assert_eq!(
        legacy_scans, 0,
        "private refusal cannot enter an unmetered legacy scanner"
    );
    assert!(!admitted);
    assert!(
        !overflow,
        "private refusal must not enter the legacy allocating scanner"
    );
    assert_eq!(backing, 0);
    assert_eq!(owners.ledger.remaining.get(), 0);
}
#[test]
fn duplicate_provider_names_and_aliases_remain_identity_distinct_and_layout_drift_rederives() {
    let second = PROVIDER
        .replace("function_layout.provider", "second_layout.provider")
        .replace("@id(\"", "@id(\"second.");
    let caller = format!(
        "{CALLER}\nuse function @id(\"second.read\") from second_layout.provider as read_again;\n"
    );
    let parsed = [
        crate::parse(&caller, "caller.spx").unwrap(),
        crate::parse(PROVIDER, "provider.spx").unwrap(),
        crate::parse(&second, "second.spx").unwrap(),
    ];
    let authored = index_authored(&parsed).unwrap();
    let layout = LayoutClosure::derive(&parsed[0], &authored, &parsed).unwrap();
    for id in ["read", "second.read"] {
        let target = authored.get(id).unwrap();
        let parameter = &target.function.unwrap().params[0].ty;
        assert_eq!(
            layout
                .signature_owner(&parsed[0], target, parameter, &authored, &parsed)
                .unwrap(),
            Some(true)
        );
    }
    assert_ne!(
        layout.alias(&parsed[0], "report"),
        layout.alias(&parsed[0], "second.report")
    );
    let duplicate = caller.replace("as read_again;", "as read;");
    assert!(crate::workspace_graph::build_owned(vec![
        crate::workspace_graph::WorkspaceSource {
            path: "caller.spx".into(),
            source: duplicate
        },
        crate::workspace_graph::WorkspaceSource {
            path: "provider.spx".into(),
            source: PROVIDER.into()
        },
        crate::workspace_graph::WorkspaceSource {
            path: "second.spx".into(),
            source: second
        }
    ])
    .is_err());
    let changed = PROVIDER.replace("@id(\"config.text\")", "@id(\"config.changed\")");
    let changed = [
        crate::parse(CALLER, "caller.spx").unwrap(),
        crate::parse(&changed, "provider.spx").unwrap(),
    ];
    let current = index_authored(&changed).unwrap();
    let current_layout = LayoutClosure::derive(&changed[0], &current, &changed).unwrap();
    assert!(layout
        .signature_owner(
            &parsed[0],
            current.get("make").unwrap(),
            &current.get("make").unwrap().function.unwrap().return_type,
            &current,
            &changed
        )
        .is_err());
    // A retained module signature must be reconstructed against the changed
    // source field identities, independently of the private ownership directory.
    let baseline = sources();
    let old_authored = index_authored(&baseline).unwrap();
    let old_modules = baseline
        .iter()
        .map(|p| {
            let synthetic = crate::workspace_graph::expected_projection::synthetic_program(
                p,
                &old_authored,
                &baseline,
            )
            .unwrap();
            (p.module.clone(), crate::hir::resolve(&synthetic).unwrap())
        })
        .collect::<Vec<_>>();
    crate::workspace_graph::validation::validate_stub_signatures(&baseline, &old_modules).unwrap();
    assert_eq!(
        crate::workspace_graph::validation::validate_stub_signatures(&changed, &old_modules)
            .unwrap_err()[0]
            .code,
        "SPX-G173"
    );
    let mut retained =
        crate::workspace_graph::validation::WorkspaceValidationIndex::new(&changed).unwrap();
    let staged = old_modules
        .iter()
        .try_for_each(|(module, program)| retained.record_module(module, program, &changed));
    let rejected = match staged {
        Err(error) => error,
        Ok(()) => retained.validate_stub_signatures(&changed).unwrap_err(),
    };
    assert_eq!(rejected[0].code, "SPX-G173");
    let fresh = changed
        .iter()
        .map(|p| {
            let synthetic = crate::workspace_graph::expected_projection::synthetic_program(
                p, &current, &changed,
            )
            .unwrap();
            (p.module.clone(), crate::hir::resolve(&synthetic).unwrap())
        })
        .collect::<Vec<_>>();
    crate::workspace_graph::validation::validate_stub_signatures(&changed, &fresh).unwrap();
    assert_eq!(
        current_layout
            .signature_owner(
                &changed[0],
                current.get("make").unwrap(),
                &current.get("make").unwrap().function.unwrap().return_type,
                &current,
                &changed
            )
            .unwrap(),
        Some(true)
    );
}
