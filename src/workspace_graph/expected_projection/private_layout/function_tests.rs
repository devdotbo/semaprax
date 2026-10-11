use super::{authority::PrivateTypes, LayoutClosure};
use crate::hir::ResolvedTypeDeclarationKind;
use crate::workspace_graph::{build_owned, index_authored, WorkspaceSource};
const PROVIDER: &str = include_str!("function_provider.spx");
const CALLER: &str = include_str!("function_caller.spx");
fn programs(caller: &str, provider: &str) -> [crate::ast::Program; 2] {
    [
        crate::parse(caller, "caller.spx").unwrap(),
        crate::parse(provider, "provider.spx").unwrap(),
    ]
}
fn sources(caller: &str, provider: &str) -> Vec<WorkspaceSource> {
    programs(caller, provider)
        .iter()
        .map(|p| WorkspaceSource {
            path: p.path.clone(),
            source: crate::format::canonical(p),
        })
        .collect()
}
#[test]
fn function_only_forwarding_and_borrowed_projection_reconstruct_only_signature_layouts() {
    let parsed = programs(CALLER, PROVIDER);
    let authored = index_authored(&parsed).unwrap();
    let layout = LayoutClosure::derive(&parsed[0], &authored, &parsed).unwrap();
    let ids = layout
        .entries
        .iter()
        .map(|e| e.declaration.stable_id.as_str())
        .collect::<Vec<_>>();
    assert_eq!(ids, ["config", "report", "row"]);
    let private = PrivateTypes::derive(&parsed[0], &parsed, &authored).unwrap();
    for id in &ids {
        assert!(private
            .contains(id, Some("function_layout.provider"))
            .unwrap());
    }
    for id in ["sibling", "ancestor", "make", "read", "forward"] {
        assert!(!private
            .contains(id, Some("function_layout.provider"))
            .unwrap());
    }
    let build = build_owned(sources(CALLER, PROVIDER)).unwrap();
    assert!(!build.edges.iter().any(
        |e| matches!(e.kind, "type_import" | "type_reference") && e.caller_path == "caller.spx"
    ));
    let linked = build
        .linked_owned_data_api_program_with_roots("function_layout.app", &[])
        .unwrap();
    crate::hir::validate(&linked).unwrap();
    assert!(linked.types.iter().any(|t| t.id.as_str() == "report"));
    let evaluated =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "app.main", 100_000).unwrap();
    assert!(matches!(
        evaluated.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
    ));
    super::function_runtime::run(&linked, false);
    let failed = PROVIDER.replace(
        "fn forward(value: own Report) -> Report\n{",
        "fn forward(value: own Report) -> Report\nensures false\n{",
    );
    assert_ne!(failed, PROVIDER);
    let failed = build_owned(sources(CALLER, &failed))
        .unwrap()
        .linked_owned_data_api_program_with_roots("function_layout.app", &[])
        .unwrap();
    crate::hir::validate(&failed).unwrap();
    super::function_runtime::run(&failed, true);
}
#[test]
fn function_private_types_do_not_grant_names_siblings_ancestors_or_foreign_roots() {
    for declaration in [
        "@id(\"bad\") fn bad(value:own Report)->i64 {0}",
        "@id(\"bad\") fn bad()->i64 {let value=Config {text:\"bad\",n:0};0}",
        "@id(\"bad\") fn bad()->i64 {let value=make();match own value {Report {config,rows}=>0,}}",
        "@id(\"bad\") fn bad(value:Sibling)->i64 {0}",
        "@id(\"bad\") fn bad(value:own Ancestor)->i64 {0}",
        "@id(\"bad\") fn bad(value:own _spx_layout_0)->i64 {0}",
        "@id(\"bad\") fn bad()->i64 {let value=_spx_layout_0 {text:\"bad\",n:0};0}",
    ] {
        let caller = format!("{CALLER}\n{declaration}\n");
        assert!(
            build_owned(sources(&caller, PROVIDER)).is_err(),
            "{declaration}"
        );
    }
    for provider in [
        PROVIDER.replace("config: Config", "config: Ancestor"),
        PROVIDER.replace("@id(\"config.text\")", ""),
        PROVIDER.replace(
            "text: string,\n    @id(\"config.n\")",
            "text: str,\n    @id(\"config.n\")",
        ),
        PROVIDER.replace("rows: Vec<Row>", "rows: Vec<Vec<Row>>"),
    ] {
        assert_ne!(provider, PROVIDER);
        assert!(build_owned(sources(CALLER, &provider)).is_err());
    }
    let parsed = programs(CALLER, PROVIDER);
    // A stale map with identical text/IDs cannot grant a current provider root.
    let authored = index_authored(&parsed).unwrap();
    let copied = parsed.clone();
    assert!(LayoutClosure::derive(&parsed[0], &authored, &copied).is_err());
    let local=format!("{CALLER}\n@id(\"local.config\") record Config {{@id(\"local.config.n\") n:i64,}}\n@id(\"local.row\") record Row {{@id(\"local.row.n\") n:i64,}}\n");
    build_owned(sources(&local, PROVIDER)).unwrap();
    let explicit = CALLER.replacen(
        "module function_layout.app;",
        "module function_layout.app;\nuse type @id(\"config\") from function_layout.provider as RenamedConfig;",
        1,
    );
    assert_ne!(explicit, CALLER);
    let named = programs(&explicit, PROVIDER);
    let names = index_authored(&named).unwrap();
    let closure = LayoutClosure::derive(&named[0], &names, &named).unwrap();
    assert_eq!(closure.alias(&named[0], "config"), Some("RenamedConfig"));
    build_owned(sources(&explicit, PROVIDER)).unwrap();
    let mut foreign = parsed[0].clone();
    foreign.module_uses[0].target_module = "foreign.provider".into();
    assert!(PrivateTypes::derive(&foreign, &parsed, &authored).is_err());
    let mut absent = parsed[0].clone();
    absent.module_uses[0].persistent_id = "sibling".into();
    assert!(PrivateTypes::derive(&absent, &parsed, &authored).is_err());
}
#[test]
fn function_signature_closure_and_authority_reservations_are_exact_and_stale_fields_refuse() {
    let parsed = programs(CALLER, PROVIDER);
    let authored = index_authored(&parsed).unwrap();
    for authority in 0..3 {
        let run = || {
            if authority == 1 {
                PrivateTypes::derive(&parsed[0], &parsed, &authored).map(|_| ())
            } else if authority == 2 {
                super::signature_owner(
                    &parsed[0],
                    "function_layout.provider",
                    &parsed[1].functions[0].return_type,
                    &authored,
                    &parsed,
                )
                .filter(|owner| *owner)
                .map(|_| ())
                .ok_or_else(super::refusal)
            } else {
                LayoutClosure::derive(&parsed[0], &authored, &parsed).map(|_| ())
            }
        };
        let (answer, overflow, used) = crate::bounded_output::with_limit_usage(usize::MAX, run);
        answer.unwrap();
        assert!(!overflow);
        assert!(used > 0);
        let (answer, overflow, _) = crate::bounded_output::with_limit_usage(used, run);
        answer.unwrap();
        assert!(!overflow);
        let (answer, overflow, _) = crate::bounded_output::with_limit_usage(used - 1, run);
        assert!(answer.is_err());
        assert!(overflow);
    }
    let mut modules = parsed
        .iter()
        .map(|p| {
            let synthetic = crate::workspace_graph::expected_projection::synthetic_program(
                p, &authored, &parsed,
            )
            .unwrap();
            (p.module.clone(), crate::hir::resolve(&synthetic).unwrap())
        })
        .collect::<Vec<_>>();
    crate::workspace_graph::validation::validate_stub_signatures(&parsed, &modules).unwrap();
    let report = modules[0]
        .1
        .types
        .iter_mut()
        .find(|t| t.id.as_str() == "report")
        .unwrap();
    let ResolvedTypeDeclarationKind::Record { fields } = &mut report.kind else {
        panic!("record")
    };
    fields[0].ty = crate::hir::ResolvedType::I64;
    assert_eq!(
        crate::workspace_graph::validation::validate_stub_signatures(&parsed, &modules)
            .unwrap_err()[0]
            .code,
        "SPX-G173"
    );
    let mut compact =
        crate::workspace_graph::validation::WorkspaceValidationIndex::new(&parsed).unwrap();
    for (module, program) in &modules {
        compact.record_module(module, program, &parsed).unwrap();
    }
    assert_eq!(
        compact.validate_stub_signatures(&parsed).unwrap_err()[0].code,
        "SPX-G173"
    );
}

#[test]
fn decoded_ready_wire_and_cleanup_match_complete_imports_with_private_descendants_omitted() {
    let complete = include_str!("ready_caller.spx");
    let provider = format!("{PROVIDER}{}", include_str!("ready_extension.spx"));
    let mut inferred = complete.to_owned();
    for (id, name) in [("report", "Report"), ("config", "Config"), ("row", "Row")] {
        let import = format!("use type @id(\"{id}\") from function_layout.provider as {name};\n");
        assert_eq!(inferred.matches(&import).count(), 1);
        inferred = inferred.replace(&import, "");
    }
    for caller in [complete, inferred.as_str()] {
        for ready in [true, false] {
            let caller = if ready {
                caller.to_owned()
            } else {
                let changed = caller.replace("let input = [0u8];", "let input = [];");
                assert_ne!(changed, caller);
                changed
            };
            let build = build_owned(sources(&caller, &provider)).unwrap();
            let type_imports = build
                .edges
                .iter()
                .filter(|edge| edge.caller_path == "caller.spx" && edge.kind == "type_import")
                .collect::<Vec<_>>();
            assert_eq!(
                type_imports.len(),
                if caller.contains("as Report;") { 4 } else { 1 }
            );
            assert!(type_imports.iter().any(|edge| edge.target == "decoded"));
            if !caller.contains("as Report;") {
                assert!(type_imports.iter().all(|edge| edge.target == "decoded"));
            }
            let linked = build
                .linked_owned_data_api_program_with_roots("function_layout.app", &[])
                .unwrap();
            crate::hir::validate(&linked).unwrap();
            for id in ["decoded", "report", "config", "row"] {
                assert!(linked.types.iter().any(|ty| ty.id.as_str() == id));
            }
            super::function_runtime::run_owned_expected(&linked, false, if ready { 42 } else { 7 });
        }
        let failed = provider.replace(
            "fn encode(value: borrow Report) -> string\n{",
            "fn encode(value: borrow Report) -> string\nensures false\n{",
        );
        assert_ne!(failed, provider);
        let linked = build_owned(sources(caller, &failed))
            .unwrap()
            .linked_owned_data_api_program_with_roots("function_layout.app", &[])
            .unwrap();
        crate::hir::validate(&linked).unwrap();
        super::function_runtime::run_owned_expected(&linked, true, 42);
    }
    let parsed = programs(&inferred, &provider);
    let authored = index_authored(&parsed).unwrap();
    let closure = LayoutClosure::derive(&parsed[0], &authored, &parsed).unwrap();
    assert_eq!(
        closure
            .entries
            .iter()
            .map(|e| e.declaration.stable_id.as_str())
            .collect::<Vec<_>>(),
        ["config", "report", "row"]
    );
    let missing_root = inferred.replace(
        "use type @id(\"decoded\") from function_layout.provider as Decoded;",
        "",
    );
    assert!(build_owned(sources(&missing_root, &provider)).is_err());
    let copied = parsed.clone();
    assert!(LayoutClosure::derive(&parsed[0], &authored, &copied).is_err());
    for changed in [
        provider.replace("@id(\"config.text\")", ""),
        provider.replace("rows: Vec<Row>", "rows: Vec<Vec<Row>>"),
        provider.replace("config: Config", "config: Report"),
    ] {
        assert_ne!(changed, provider);
        assert!(build_owned(sources(&inferred, &changed)).is_err());
    }
    // A fresh provider body is used, rather than an old stub or old wire value.
    let drift = provider.replace("text: \"row\"", "text: \"changed\"");
    assert_ne!(drift, provider);
    let changed = build_owned(sources(&inferred, &drift))
        .unwrap()
        .linked_owned_data_api_program_with_roots("function_layout.app", &[])
        .unwrap();
    crate::hir::validate(&changed).unwrap();
    super::function_runtime::run_owned_expected(&changed, false, 0);
    // Retained private layouts must match the current complete provider layout.
    let mut modules = parsed
        .iter()
        .map(|p| {
            let synthetic = crate::workspace_graph::expected_projection::synthetic_program(
                p, &authored, &parsed,
            )
            .unwrap();
            (p.module.clone(), crate::hir::resolve(&synthetic).unwrap())
        })
        .collect::<Vec<_>>();
    crate::workspace_graph::validation::validate_stub_signatures(&parsed, &modules).unwrap();
    let ready = modules[0]
        .1
        .types
        .iter_mut()
        .find(|t| t.id.as_str() == "decoded")
        .unwrap();
    let ResolvedTypeDeclarationKind::Variant { cases } = &mut ready.kind else {
        panic!("variant")
    };
    cases
        .iter_mut()
        .find(|case| case.id.as_str() == "decoded.ready")
        .unwrap()
        .fields[0]
        .ty = crate::hir::ResolvedType::I64;
    assert_eq!(
        crate::workspace_graph::validation::validate_stub_signatures(&parsed, &modules)
            .unwrap_err()[0]
            .code,
        "SPX-G173"
    );
    let mut retained =
        crate::workspace_graph::validation::WorkspaceValidationIndex::new(&parsed).unwrap();
    for (module, program) in &modules {
        retained.record_module(module, program, &parsed).unwrap();
    }
    assert_eq!(
        retained.validate_stub_signatures(&parsed).unwrap_err()[0].code,
        "SPX-G173"
    );
}

/// Whether a diagnostic message or help contains, as an exact identifier token,
/// an alias that the caller's own closure minted. Authored spellings with the
/// same prefix are not aliases and are not matched.
fn exposes_alias(diagnostics: &[crate::diagnostic::Diagnostic], aliases: &[String]) -> bool {
    let token = |text: &str, alias: &str| {
        text.match_indices(alias).any(|(start, _)| {
            let identifier = |c: char| c.is_ascii_alphanumeric() || c == '_';
            !text[..start].ends_with(identifier)
                && !text[start + alias.len()..].starts_with(identifier)
        })
    };
    diagnostics.iter().any(|d| {
        aliases.iter().any(|alias| {
            token(&d.message, alias) || d.help.as_deref().is_some_and(|help| token(help, alias))
        })
    })
}

#[test]
fn private_layout_diagnostics_present_provider_provenance_and_only_legal_imports() {
    let provider = format!("{PROVIDER}{}", include_str!("ready_extension.spx"));
    let mut missing = include_str!("ready_caller.spx").to_owned();
    for (id, name) in [
        ("decoded", "Decoded"),
        ("report", "Report"),
        ("config", "Config"),
        ("row", "Row"),
    ] {
        let import = format!("use type @id(\"{id}\") from function_layout.provider as {name};\n");
        assert_eq!(missing.matches(&import).count(), 1);
        missing = missing.replace(&import, "");
    }
    let aliases = |caller: &str| {
        let parsed = programs(caller, &provider);
        let authored = index_authored(&parsed).unwrap();
        let layout = LayoutClosure::derive(&parsed[0], &authored, &parsed).unwrap();
        layout
            .entries
            .iter()
            .map(|e| (e.declaration.stable_id.clone(), e.alias.clone()))
            .collect::<Vec<_>>()
    };
    // The function-only `decode` signature still retains the exact private
    // outcome layout; no authored source names it.
    let retained = aliases(&missing);
    assert_eq!(
        retained.iter().map(|(id, _)| id.as_str()).collect::<Vec<_>>(),
        ["config", "decoded", "report", "row"]
    );
    let minted = retained.into_iter().map(|(_, a)| a).collect::<Vec<_>>();
    let diagnostics = build_owned(sources(&missing, &provider))
        .err()
        .expect("unimported variant patterns are rejected");
    assert!(!exposes_alias(&diagnostics, &minted), "{diagnostics:?}");
    for pattern in ["`Decoded::Error`", "`Decoded::Ready`"] {
        assert!(
            diagnostics
                .iter()
                .any(|d| d.code == "SPX-M103" && d.message.contains(pattern)),
            "{pattern}: {diagnostics:?}"
        );
    }
    let missing_case = diagnostics
        .iter()
        .find(|d| d.code == "SPX-M101")
        .expect("the private variant match is still non-exhaustive");
    assert!(
        missing_case.message.contains("`Ready { .. }`")
            && missing_case
                .message
                .contains("`Decoded` (@id(\"decoded\") from function_layout.provider)"),
        "{missing_case:?}"
    );
    let help = missing_case.help.as_deref().expect("legal explicit import");
    let import = help.split('`').nth(1).expect("quoted import");
    assert_eq!(
        import,
        "use type @id(\"decoded\") from function_layout.provider as Decoded;"
    );
    // Applying the offered import is the explicit-root control: it succeeds.
    let repaired = missing.replacen("use function", &format!("{import}\nuse function"), 1);
    assert_ne!(repaired, missing);
    let linked = build_owned(sources(&repaired, &provider))
        .unwrap()
        .linked_owned_data_api_program_with_roots("function_layout.app", &[])
        .unwrap();
    crate::hir::validate(&linked).unwrap();
    // A caller already spelling the provider name gets no fabricated alias.
    let taken = format!(
        "{missing}\n@id(\"local.decoded\") record Decoded {{ @id(\"local.decoded.n\") n: i64, }}\n"
    );
    let minted = aliases(&taken).into_iter().map(|(_, a)| a).collect::<Vec<_>>();
    let diagnostics = build_owned(sources(&taken, &provider)).err().unwrap();
    assert!(!exposes_alias(&diagnostics, &minted), "{diagnostics:?}");
    let help = diagnostics
        .iter()
        .find(|d| d.code == "SPX-M101")
        .and_then(|d| d.help.as_deref())
        .expect("provider-supported import action");
    assert!(
        help.contains("@id(\"decoded\") from function_layout.provider") && !help.contains(" as "),
        "{help}"
    );
    // Inferred record values keep provider context in record-pattern and call
    // diagnostics, and an authored colliding spelling stays authored.
    for (declaration, code) in [
        (
            "@id(\"bad\") fn bad()->i64 {let value=make();match own value {Report {config,rows}=>0,}}",
            None,
        ),
        (
            "@id(\"bad\") fn bad()->i64 {read(5)}",
            Some("SPX-T205"),
        ),
        ("@id(\"bad\") fn bad(value:own _spx_layout_0)->i64 {0}", None),
        ("@id(\"bad\") fn bad()->bool {make()}", Some("SPX-T103")),
    ] {
        let caller = format!("{CALLER}\n{declaration}\n");
        let minted = aliases(&caller).into_iter().map(|(_, a)| a).collect::<Vec<_>>();
        if declaration.contains("_spx_layout_0") {
            assert!(!minted.iter().any(|alias| alias == "_spx_layout_0"));
        }
        let diagnostics = build_owned(sources(&caller, PROVIDER))
            .err()
            .expect(declaration);
        assert!(!exposes_alias(&diagnostics, &minted), "{diagnostics:?}");
        for d in &diagnostics {
            if d.message.contains("record pattern `Report`") || Some(d.code) == code {
                assert!(
                    d.message
                        .contains("Report [unimported @id(\"report\") from function_layout.provider]"),
                    "{d:?}"
                );
            }
        }
        if let Some(code) = code {
            assert!(diagnostics.iter().any(|d| d.code == code), "{diagnostics:?}");
        }
    }
    // A private descendant nested in a payload is presented in place, and the
    // authored declared type stays authored.
    let caller = format!(
        "{CALLER}\n@id(\"bad\") fn bad()->i64 {{let value=make();let rows:bool=value.rows;0}}\n"
    );
    let minted = aliases(&caller).into_iter().map(|(_, a)| a).collect::<Vec<_>>();
    let diagnostics = build_owned(sources(&caller, PROVIDER)).err().unwrap();
    assert!(!exposes_alias(&diagnostics, &minted), "{diagnostics:?}");
    let binding = diagnostics
        .iter()
        .find(|d| d.code == "SPX-T232")
        .expect("declared binding mismatch");
    assert_eq!(
        binding.message,
        "declared binding type `bool` does not accept value type \
         `Vec<Row [unimported @id(\"row\") from function_layout.provider]>`"
    );
}
