//! Real checked cache cold/warm and current imported-provider drift for Take.
use super::*;
#[test]
fn named_string_transfer_checked_cache_keeps_cold_warm_provider_drift_and_recovery() {
    let fixture = Fixture::new();
    let original = fixture.read("src/app.spx");
    assert!(original.contains("add(multiply(6, 7), subtract(divide(4, 2), 2))"));
    fixture.write("src/app.spx", &original.replace(
        "add(multiply(6, 7), subtract(divide(4, 2), 2))",
        "{let text=\"é\";let moved=string_take(text);add(multiply(6, 7), subtract(divide(4, 2), 2))+string_len(moved)}"));
    let manifest = fixture.manifest();
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    let cold = cache.build(&manifest, &fixture.sources()).unwrap();
    work(&cold, 4, 0);
    same(cold.revision(), fixture.cold().unwrap());
    let warm = cache.build(&manifest, &fixture.sources()).unwrap();
    work(&warm, 0, 4);
    same(warm.revision(), fixture.cold().unwrap());
    assert!(warm
        .revision()
        .semantic_graph()
        .contains("core.string.take"));
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
    let cached = errors(cache.build(&manifest, &fixture.sources()));
    let independent = errors(fixture.cold());
    assert!(!cached.is_empty());
    assert_eq!(format!("{cached:?}"), format!("{independent:?}"));
    fixture.write("src/core.spx", &provider);
    let restored = cache.build(&manifest, &fixture.sources()).unwrap();
    same(restored.revision(), fixture.cold().unwrap());
    assert_eq!(
        restored.revision().project_revision(),
        cold.revision().project_revision()
    );
    same(
        cache
            .build(&manifest, &fixture.sources())
            .unwrap()
            .revision(),
        fixture.cold().unwrap(),
    );
}
