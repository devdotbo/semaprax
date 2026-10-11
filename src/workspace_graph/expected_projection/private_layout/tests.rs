use super::*;
use crate::workspace_graph::{build_owned, index_authored, WorkspaceSource};
const PROVIDER: &str = r#"module closure.provider;
@id("config") record Config { @id("config.label") label:string, }
@id("row") record Row { @id("row.sku") sku:string, }
@id("report") record Report { @id("report.config") config:Config, @id("report.rows") rows:Vec<Row>, }
@id("decoded") variant Decoded {
 @id("ready") Ready { @id("ready.value") value:Report, },
 @id("error") Error { @id("error.code") code:i64, @id("error.offset") offset:usize, @id("error.field") field:i64, },
}
@id("decode") fn decode(input:borrow Slice<u8>)->Decoded { Decoded::Error { code:7, offset:0usize, field:0 } }
"#;
const APP: &str = r#"module closure.app;
use type @id("decoded") from closure.provider as Decoded;
use function @id("decode") from closure.provider as decode;
@id("main") fn main()->i64 {
 let input=[0u8]; let decoded=decode(array_as_slice(input));
 match own decoded { Decoded::Error {code,offset,field}=>code, Decoded::Ready {value}=>0, }
}
"#;
fn programs(app: &str, provider: &str) -> [Program; 2] {
    [
        crate::parse(app, "app.spx").unwrap(),
        crate::parse(provider, "provider.spx").unwrap(),
    ]
}
fn sources(app: &str, provider: &str) -> Vec<WorkspaceSource> {
    programs(app, provider)
        .iter()
        .map(|p| WorkspaceSource {
            path: p.path.clone(),
            source: crate::format::canonical(p),
        })
        .collect()
}
#[test]
fn explicit_root_retains_private_descendants_and_only_authored_edges() {
    let build = build_owned(sources(APP, PROVIDER)).unwrap();
    let linked = build
        .linked_owned_data_api_program_with_roots("closure.app", &[])
        .unwrap();
    crate::hir::validate(&linked).unwrap();
    for id in ["decoded", "report", "config", "row", crate::prelude::VEC_ID] {
        assert!(linked.types.iter().any(|t| t.id.as_str() == id), "{id}");
    }
    let vector = crate::hir::DeclarationId::new(crate::prelude::VEC_ID);
    assert_eq!(
        linked.declarations.type_parameters(&vector).unwrap().len(),
        1
    );
    let imports = build
        .edges
        .iter()
        .filter(|e| {
            e.caller_path == "app.spx" && matches!(e.kind, "function_import" | "type_import")
        })
        .collect::<Vec<_>>();
    assert_eq!(imports.len(), 2);
    assert!(imports
        .iter()
        .all(|e| e.target == "decoded" || e.target == "decode"));
    let evaluated =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "main", 100_000).unwrap();
    assert!(matches!(
        evaluated.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(7)
    ));
    let mut forged = linked.clone();
    assert!(forged.declarations.remove_type_parameters_for_test(&vector));
    assert_eq!(crate::hir::validate(&forged).unwrap_err().code, "SPX-H006");
    // An unrelated provider declaration must not convert an existing explicit
    // scalar-payload outcome into the narrow inferred-root profile.
    let extended = format!(
        "{PROVIDER}\n{}",
        r#"@id("encoded") variant Encoded {
 @id("encoded.ok") Ok { @id("encoded.text") text:string, },
 @id("encoded.no") Refused { @id("encoded.required") required:usize, },
}
@id("unused") record Unused { @id("unused.n") n:i64, }
"#
    );
    let mixed = APP.replace(
        "@id(\"main\")",
        "use type @id(\"encoded\") from closure.provider as Encoded;\n@id(\"main\")",
    );
    assert!(build_owned(sources(&mixed, &extended)).is_ok());
    let explicit = "module closure.app; use type @id(\"encoded\") from closure.provider as Encoded; @id(\"main\") fn main()->i64 { 7 }";
    assert!(build_owned(sources(explicit, &extended)).is_ok());
    let missing = APP.replace(
        "use type @id(\"decoded\") from closure.provider as Decoded;",
        "",
    );
    assert!(build_owned(sources(&missing, PROVIDER))
        .err()
        .unwrap()
        .iter()
        .any(|d| d.code == "SPX-G172"));
}
#[test]
fn private_names_avoid_all_authored_refs_without_exposing_child_names() {
    let app = APP.replace(
        "@id(\"main\")",
        r#"@id("local.config") record Config { @id("local.config.n") n:i64, }
@id("local.row") record Row { @id("local.row.n") n:i64, }
@id("unreachable") fn unreachable(value:_spx_layout_90)->i64 { 0 }
@id("main")"#,
    );
    let parsed = programs(&app, PROVIDER);
    let index = index_authored(&parsed).unwrap();
    let closure = LayoutClosure::derive(&parsed[0], &index, &parsed).unwrap();
    assert_eq!(closure.entries.len(), 3);
    assert!(closure.entries.iter().all(|e| e
        .alias
        .strip_prefix("_spx_layout_")
        .unwrap()
        .parse::<u64>()
        .unwrap()
        > 90));
    // The unresolved authored annotation must remain unresolved, despite inference.
    assert!(build_owned(sources(&app, PROVIDER)).is_err());
    let collision = APP.replace(
        "@id(\"main\")",
        r#"@id("local.config") record Config { @id("local.config.n") n:i64, }
@id("local.row") record Row { @id("local.row.n") n:i64, }
@id("main")"#,
    );
    build_owned(sources(&collision, PROVIDER)).unwrap();
    for app in [
        APP.replace(
            "let input=",
            "let ghost=Config { label: \"hidden\" }; let input=",
        ),
        APP.to_owned() + "\n@id(\"named\") fn named(value:own Report)->i64{0}\n",
    ] {
        assert!(
            build_owned(sources(&app, PROVIDER)).is_err(),
            "private child became source-visible"
        );
    }
    let explicit = APP.replace(
        "@id(\"main\")",
        r#"use type @id("config") from closure.provider as Renamed;
@id("main")"#,
    );
    let parsed = programs(&explicit, PROVIDER);
    let index = index_authored(&parsed).unwrap();
    let closure = LayoutClosure::derive(&parsed[0], &index, &parsed).unwrap();
    assert_eq!(closure.alias(&parsed[0], "config"), Some("Renamed"));
    build_owned(sources(&explicit, PROVIDER)).unwrap();
}
#[test]
fn provider_layout_reconstruction_refuses_identity_shape_and_source_drift() {
    for provider in [
        PROVIDER.replace("@id(\"config.label\") ", ""),
        PROVIDER.replace("label:string", "label:str"),
        PROVIDER.replace("rows:Vec<Row>", "rows:Vec<Vec<Row>>"),
        PROVIDER.replace("config:Config", "config:Decoded"),
        PROVIDER.replace("@id(\"row\")", "@id(\"decoded\")"),
    ] {
        assert_ne!(provider, PROVIDER);
        assert!(build_owned(sources(APP, &provider)).is_err());
    }
    let drift = PROVIDER.replace("code:7", "code:9");
    let build = build_owned(sources(APP, &drift)).unwrap();
    let linked = build
        .linked_owned_data_api_program_with_roots("closure.app", &[])
        .unwrap();
    let evaluated =
        crate::interpreter::evaluate_resolved_zero_arg_i64(&linked, "main", 100_000).unwrap();
    assert!(matches!(
        evaluated.outcome,
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(9)
    ));
    let parsed = programs(APP, PROVIDER);
    let index = index_authored(&parsed).unwrap();
    let old = LayoutClosure::derive(&parsed[0], &index, &parsed).unwrap();
    let changed = programs(APP, &PROVIDER.replace("config.label", "config.changed"));
    let changed_index = index_authored(&changed).unwrap();
    let current = LayoutClosure::derive(&changed[0], &changed_index, &changed).unwrap();
    assert!(!std::ptr::eq(
        old.entries[0].declaration,
        current.entries[0].declaration
    ));
    assert!(current.entries.iter().any(|e|matches!(&e.declaration.kind,TypeDeclarationKind::Record{fields} if fields.iter().any(|f|f.stable_id=="config.changed"))));
}
#[test]
fn closure_scratch_names_and_directory_have_exact_and_one_short_reservations() {
    let parsed = programs(APP, PROVIDER);
    let index = index_authored(&parsed).unwrap();
    let (first, overflowed, used) =
        crate::bounded_output::with_limit_usage(crate::workspace_graph::MAX_BUILDER_BYTES, || {
            LayoutClosure::derive(&parsed[0], &index, &parsed)
        });
    assert!(!overflowed);
    assert_eq!(first.as_ref().unwrap().entries.len(), 3);
    drop(first);
    // Optional absence work exhaustion disables inference before any alias or
    // directory is created; it does not grant missing children an alias.
    let mut small = 1usize;
    assert_eq!(
        shape::explicit_only(&parsed[0], &index, &parsed, &mut small).unwrap(),
        Absence::WorkExhausted
    );
    assert_eq!(small, 0);
    let mut wrong_target = parsed[0].clone();
    wrong_target.module_uses[0].target_module = "wrong.provider".into();
    let mut full = WORK;
    assert!(shape::explicit_only(&wrong_target, &index, &parsed, &mut full).is_err());
    assert!(super::super::type_rewrite::rewrite(
        &mut Type::Named {
            name: "Report".into(),
            arguments: vec![]
        },
        "closure.provider",
        &parsed[0],
        &parsed
    )
    .is_err());
    let explicit_app = APP.replace("use function", "use type @id(\"report\") from closure.provider as Report; use type @id(\"config\") from closure.provider as Config; use type @id(\"row\") from closure.provider as Row; use function");
    let explicit = programs(&explicit_app, PROVIDER);
    let explicit_index = index_authored(&explicit).unwrap();
    let mut small = 1usize;
    assert_eq!(
        shape::explicit_only(&explicit[0], &explicit_index, &explicit, &mut small).unwrap(),
        Absence::WorkExhausted
    );
    assert!(super::super::type_rewrite::rewrite(
        &mut Type::Named {
            name: "Report".into(),
            arguments: vec![]
        },
        "closure.provider",
        &explicit[0],
        &explicit
    )
    .is_ok());
    assert!(used > 0);
    let (exact, overflowed, exact_used) = crate::bounded_output::with_limit_usage(used, || {
        LayoutClosure::derive(&parsed[0], &index, &parsed)
    });
    assert!(!overflowed);
    assert!(exact.is_ok());
    assert_eq!(exact_used, used);
    drop(exact);
    let (short, _, _) = crate::bounded_output::with_limit_usage(used - 1, || {
        LayoutClosure::derive(&parsed[0], &index, &parsed)
    });
    assert!(short.err().unwrap().iter().any(|d| d.code == "SPX-G171"));
    let huge=APP.replace("@id(\"main\")", "@id(\"oversized\") fn oversized(value:_spx_layout_18446744073709551615)->i64{0} @id(\"main\")");
    let parsed = programs(&huge, PROVIDER);
    let index = index_authored(&parsed).unwrap();
    assert!(LayoutClosure::derive(&parsed[0], &index, &parsed)
        .err()
        .unwrap()
        .iter()
        .any(|d| d.code == "SPX-G172"));
}
