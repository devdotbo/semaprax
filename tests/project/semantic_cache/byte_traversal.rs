//! Actual Project cache ownership; codec round-trip alone is not warm evidence.
use super::*;

fn byte_fixture() -> Fixture {
    // Retain the row-loan provider/consumer so v83 must compose with v82 facts.
    let fixture = row_loan_fixture();
    for (path, source) in checked_cache_profiles::sources(true) {
        fixture.write(path, &source);
    }
    fixture
}

#[test]
fn warm_byte_traversal_cache_preserves_mixed_graph_and_current_provider_refusals() {
    let fixture = byte_fixture();
    let manifest = fixture.manifest();
    let sources = || row_loan_sources(&fixture, &manifest);
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    let cold = cache.build(&manifest, &sources()).unwrap();
    work(&cold, 3, 0);
    assert!(cold
        .revision()
        .sources()
        .iter()
        .any(|source| source.source_graph_schema() == "semaprax.graph.v83"));
    same(cold.revision(), fixture.cold().unwrap());
    let warm = cache.build(&manifest, &sources()).unwrap();
    assert_eq!(work(&warm, 0, 3)["invalidated_sources"], json!([]));
    same(warm.revision(), Arc::clone(cold.revision()));

    let original = fixture.read("src/frame.spx");
    fixture.replace("src/frame.spx", "total = total + 1", "total = 1 + total");
    let edited = cache.build(&manifest, &sources()).unwrap();
    let report: Value = serde_json::from_str(edited.to_json()).unwrap();
    assert_eq!(report["work"]["modules_parsed"], 1);
    assert_eq!(report["invalidated_sources"], json!(["src/frame.spx"]));
    same(edited.revision(), fixture.cold().unwrap());

    // The unchanged consumer cannot confer authority on a mutable current view.
    fixture.replace(
        "src/frame.spx",
        "let view = input;",
        "let mut view = input;",
    );
    let cached = errors(cache.build(&manifest, &sources()));
    let cold_errors = errors(fixture.cold());
    assert_eq!(format!("{cached:?}"), format!("{cold_errors:?}"));
    assert!(cached.iter().any(|error| error.code == "SPX-T284"));

    // A refusal must not replace the last complete immutable checked state.
    fixture.write("src/frame.spx", &original);
    let restored = cache.build(&manifest, &sources()).unwrap();
    same(restored.revision(), fixture.cold().unwrap());
    same(restored.revision(), Arc::clone(cold.revision()));
}
