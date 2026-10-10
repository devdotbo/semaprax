//! Retained authority is independently replayed, never inferred from v77.
use super::*;
const WRITER: &str = include_str!("../../../../tests/owned_data/static_utf8_writer.spx");
fn build(source: &str) -> WorkspaceGraphBuild {
    let sources = [
        ("writer.spx", source),
        (
            "support.spx",
            "module support.ready; @id(\"ready\") fn ready()->i64 {0}",
        ),
    ]
    .into_iter()
    .map(|(path, source)| WorkspaceSource {
        path: path.to_owned(),
        source: format::canonical(&crate::parse(source, path).unwrap()),
    })
    .collect();
    build_owned_retaining_sources_for_change(sources, MAX_CHANGE_BUILDER_BYTES)
        .unwrap()
        .0
}
fn writer(build: &WorkspaceGraphBuild) -> &WorkspaceResolvedModule {
    build
        .hir
        .modules
        .iter()
        .find(|module| module.module == "static.writer")
        .unwrap()
}
fn writer_mut(build: &mut WorkspaceGraphBuild) -> &mut WorkspaceResolvedModule {
    build
        .hir
        .modules
        .iter_mut()
        .find(|module| module.module == "static.writer")
        .unwrap()
}
fn refused(build: &WorkspaceGraphBuild) {
    let errors = super::super::schema(writer(build), &build.hir.modules, &build.hir.declarations)
        .unwrap_err();
    assert!(errors.iter().any(|error| error.code == "SPX-G410"));
}
#[test]
fn retained_writer_replays_source_sites_and_composes_helper_renewal() {
    let build = build(WRITER);
    let module = writer(&build);
    assert_eq!(
        schema(module, &build.hir.declarations).unwrap(),
        Some("semaprax.graph.v77")
    );
    assert_eq!(
        build.source_graph_schemas().unwrap()["writer.spx"],
        "semaprax.graph.v77"
    );
    assert_eq!(module.static_text.as_ref().unwrap().sites.len(), 2);
    assert!(module.functions.iter().any(
        |function| function.cleanup_plan.schema == crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V19
    ));
    assert_eq!(
        graph::graph_schema_from_parts_and_instances(
            &module.interfaces,
            &module.types,
            &module.functions,
            &module.function_templates,
            &module.function_instances
        )
        .unwrap_err()
        .code,
        "SPX-G410"
    );
    let changed = build_source_changed();
    assert_ne!(
        module.static_text.as_ref().unwrap().sites[0].text.as_str(),
        writer(&changed).static_text.as_ref().unwrap().sites[0]
            .text
            .as_str()
    );
}
fn build_source_changed() -> WorkspaceGraphBuild {
    build(&WRITER.replace("aé", "bé"))
}
#[test]
fn source_inventory_content_identity_namespace_and_presence_fail_closed() {
    for attack in 0..6 {
        let mut build = build(WRITER);
        let module = writer_mut(&mut build);
        let inventory = module.static_text.as_mut().unwrap();
        match attack {
            0 => inventory.sites[0].text = hir::StaticUtf8::from_text("forged"),
            1 => inventory.sites[0].path.push('x'),
            2 => inventory.sites[0].expression = module.functions[0].body.id.clone(),
            3 => inventory.path.push('x'),
            4 => inventory.sites[0].explicit = !inventory.sites[0].explicit,
            _ => module.static_text = None,
        }
        refused(&build);
    }
}
#[test]
fn retained_generic_literals_match_template_and_concrete_execution_sites() {
    let source = "module static.writer; @id(\"generic\") fn generic<T>(value:T)->T {let key=str\"é\";value} @id(\"app.main\") fn main()->i64 {generic<i64>(42)}";
    let mut build = build(source);
    let module = writer(&build);
    assert_eq!(
        schema(module, &build.hir.declarations).unwrap(),
        Some("semaprax.graph.v77")
    );
    assert_eq!(module.function_templates.len(), 1);
    assert!(!module.function_instances.is_empty());
    let module = writer_mut(&mut build);
    let replacement = module.functions[0].body.id.clone();
    let instance = &mut module.function_instances[0].function;
    let hir::ResolvedExprKind::Block { statements, .. } = &mut instance.body.kind else {
        panic!("generic body")
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("generic literal")
    };
    value.id = replacement;
    refused(&build);
}
