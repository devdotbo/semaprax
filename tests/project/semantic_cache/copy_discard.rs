//! Current cache reuse must retain the explicit statement boundary.
use super::*;
const APP: &str = r#"module calculator.app;
use function @id("calculator.add") from calculator.core as add;
@id("report") record Report {@id("report.bytes") bytes:Bytes,}
@id("outer") record Outer {@id("outer.report") report:Report,}
@id("score") fn score(value:own Report)->i64 {match own value {Report{bytes}=>i64_from_usize(byte_len(bytes_as_slice(bytes))),}}
@id("calculator.app.main") fn main()->i64 {
 let mut count=0;
 while count<3 {
  let outer=Outer{report:Report{bytes:bytes_zeroed(3usize)}};
  match own outer {Outer{report}=>{
   count=count+1;
   if count==1 {continue;}else{if count==2 {break;}else{score(report)}}
  },};
  0
 }
 add(count,40)
}
"#;
#[test]
fn warm_copy_discard_cache_preserves_current_provider_and_lexical_wire() {
    let fixture = Fixture::new();
    let ast = semaprax::parse(APP, "app.spx").unwrap();
    fixture.write("src/app.spx", &semaprax::format::canonical(&ast));
    // The owned private helper remains unavailable under ScalarV1.
    let scalar_manifest = fixture.manifest();
    let scalar_refusal = errors(
        ProjectFrontendCache::new_with_semantic_cache().build(&scalar_manifest, &fixture.sources()),
    );
    assert!(
        scalar_refusal.iter().any(|error| error.code == "SPX-G174"),
        "{scalar_refusal:?}"
    );
    // UsefulDataV2 admits ordinary private entry helpers, with no public exports.
    std::fs::write(
        fixture.0.join("semaprax.toml"),
        r#"schema = "semaprax.project.v16"
name = "calculator"
version = "0.1.0"
profile = "useful-data.v2"
entry = "calculator.app"
sources = ["src/app.spx", "src/core.spx", "src/spare.spx", "src/tests.spx"]
web_exports = []
tests = ["calculator.tests"]
"#,
    )
    .unwrap();
    let manifest = fixture.manifest();
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    let cold = cache.build(&manifest, &fixture.sources()).unwrap();
    work(&cold, 4, 0);
    same(cold.revision(), fixture.cold().unwrap());
    let entry = cold.revision().entry_program();
    let main = entry
        .functions
        .iter()
        .find(|function| function.id.as_str() == "calculator.app.main")
        .unwrap();
    let semaprax::hir::ResolvedExprKind::Block { statements, .. } = &main.body.kind else {
        panic!("entry block")
    };
    let semaprax::hir::ResolvedStatement::LexicalWhile { body, .. } = &statements[1] else {
        panic!("finite lexical loop")
    };
    let semaprax::hir::ResolvedExprKind::Block { statements, .. } = &body.kind else {
        panic!("loop block")
    };
    assert!(
        matches!(&statements[1], semaprax::hir::ResolvedStatement::CopyDiscard { value, .. }
        if matches!(&value.kind, semaprax::hir::ResolvedExprKind::Match { .. }))
    );
    assert!(cold
        .revision()
        .sources()
        .iter()
        .any(|s| s.source_graph_schema() == "semaprax.graph.v84"));
    let warm = cache.build(&manifest, &fixture.sources()).unwrap();
    work(&warm, 0, 4);
    same(warm.revision(), Arc::clone(cold.revision()));
    let provider = fixture.read("src/core.spx");
    assert!(provider.contains("left + right"));
    fixture.write(
        "src/core.spx",
        &provider.replace("left + right", "left + right + 1"),
    );
    let drift = cache.build(&manifest, &fixture.sources()).unwrap();
    assert_ne!(
        drift.revision().project_revision(),
        warm.revision().project_revision()
    );
    same(drift.revision(), fixture.cold().unwrap());
    fixture.write(
        "src/core.spx",
        &provider.replace(
            "fn add(left: i64, right: i64) -> i64",
            "fn add(left: i64, right: i64) -> bool",
        ),
    );
    let current = errors(cache.build(&manifest, &fixture.sources()));
    assert!(!current.is_empty());
    assert_eq!(
        format!("{current:?}"),
        format!("{:?}", errors(fixture.cold()))
    );
    fixture.write("src/core.spx", &provider);
    let restored = cache.build(&manifest, &fixture.sources()).unwrap();
    same(restored.revision(), fixture.cold().unwrap());
    same(restored.revision(), Arc::clone(cold.revision()));
}
