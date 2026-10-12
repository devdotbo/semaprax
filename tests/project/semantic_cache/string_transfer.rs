//! Real checked cache cold/warm and current imported-provider drift for Take.
use super::*;
#[test]
fn named_string_transfer_checked_cache_keeps_cold_warm_provider_drift_and_recovery() {
    let fixture = Fixture::new();
    // The entry module stays inside the Public Scalar Export Profile, which
    // refuses a String binding. As in the row-loan and byte-traversal fixtures,
    // the owned String work lives in the tests module's own non-exported helper.
    let original = fixture.read("src/tests.spx");
    assert!(original.contains("is_negative(-1) && not(false)"));
    fixture.write(
        "src/tests.spx",
        &(original.replace(
            "is_negative(-1) && not(false)",
            "is_negative(-1) && not(false) && moved_len() == 2",
        ) + "\n@id(\"calculator.tests.moved\") fn moved_len() -> i64 {let text=\"é\";let moved=string_take(text);string_len(moved)}\n"),
    );
    let manifest = fixture.manifest();
    let mut cache = ProjectFrontendCache::new_with_semantic_cache();
    let cold = cache.build(&manifest, &fixture.sources()).unwrap();
    work(&cold, 4, 0);
    same(cold.revision(), fixture.cold().unwrap());
    let warm = cache.build(&manifest, &fixture.sources()).unwrap();
    work(&warm, 0, 4);
    same(warm.revision(), fixture.cold().unwrap());
    // The workspace graph inventories declarations; the actual intrinsic
    // call belongs to the current owning tests-program expression tree.
    let program = warm.revision().test_program();
    assert_eq!(program, cold.revision().test_program());
    let helper = program.functions.iter()
        .find(|function| function.id.as_str() == "calculator.tests.moved").unwrap();
    let semaprax::hir::ResolvedExprKind::Block { statements, .. } = &helper.body.kind
        else { panic!("current moved helper lost its authored block") };
    let value = statements.iter().find_map(|statement| match statement {
        semaprax::hir::ResolvedStatement::Let { binding, value, .. }
            if binding.name == "moved" => Some(value),
        _ => None,
    }).unwrap();
    let semaprax::hir::ResolvedExprKind::Call { callee, instance, args, type_arguments } = &value.kind
        else { panic!("current moved initializer lost its exact intrinsic call") };
    assert_eq!(callee.as_str(), "core.string.take");
    assert!(instance.is_none());
    assert!(type_arguments.is_empty());
    assert_eq!(args.len(), 1);
    assert_eq!(value.ty, semaprax::hir::ResolvedType::String);
    assert_eq!(value.ownership, semaprax::hir::OwnershipMode::Own);
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
