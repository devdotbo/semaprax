//! Functional byte helpers retain one owner without any helper allocation.
use semaprax::{format, graph, hir, interpreter};
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = include_str!("byte_helper_renewal.spx");

#[test]
fn byte_helper_renewal_canonical_graph_and_physical_engines_preserve_one_owner() {
    let checked = semaprax::check(SOURCE, "byte-helper-owning.spx").unwrap();
    let canonical = format::canonical(&checked);
    let roundtrip = semaprax::check(&canonical, "byte-helper-owning.spx").unwrap();
    assert_eq!(format::canonical(&roundtrip), canonical);
    let resolved = hir::resolve(&roundtrip).unwrap();
    hir::validate(&resolved).unwrap();
    let main = resolved
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    assert_eq!(main.cleanup_plan.schema, "semaprax.cleanup-plan.v19");
    assert!(main.cleanup_plan.lexical_control.is_none());
    let frozen = graph::to_legacy_json(&roundtrip).unwrap_err();
    assert!(frozen
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-G410"));
    let json = graph::to_json(&roundtrip).unwrap();
    graph::verify_json(&roundtrip, &json).unwrap();
    let document: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(document["schema"], "semaprax.graph.v76");
    assert_eq!(
        document["byte_helper_renewal"]["schema"],
        "semaprax.byte-helper-renewal.v1"
    );
    assert_eq!(
        document["byte_helper_renewal"]["updates"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|update| update["function"] == "app.main")
            .count(),
        2
    );
    let root = std::env::temp_dir().join(format!(
        "semaprax-byte-helper-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("program.spx");
    std::fs::write(&path, SOURCE).unwrap();
    for _ in 0..3 {
        let outcome = interpreter::interpret(
            &path,
            "app.main",
            &[],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        assert!(outcome.returned);
        let envelope: serde_json::Value = serde_json::from_str(&outcome.envelope).unwrap();
        assert_eq!(envelope["payload"]["outcome"]["value"], "42");
    }
    // Existing owning probes count all physical allocations and reject stale handles.
    let generated = semaprax::codegen::emit_c(&checked).unwrap();
    assert!(!generated.contains("SPX_VEC_AUTHORITY_CAPACITY"));
    assert!(!generated.contains("struct spx_vec_authority_entry"));
    assert!(generated.contains("spx_bytes_require_valid"));
    assert!(generated.contains("spx_bytes_drop"));
    crate::owned_collection_outcome::run_native_generated_with_allocation_count(
        generated,
        &root,
        Some(2), // output and the untouched owner; helper renewals allocate nothing.
    );
    crate::owned_vec_bytes_runtime::run_wasm_expected(SOURCE, 0, 2, "none", 42);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn helper_returned_owner_is_protected_until_postconditions_and_failure_is_sticky() {
    let source = SOURCE.replace(
        "requires index<byte_len(bytes_as_slice(output)) {",
        "requires index<byte_len(bytes_as_slice(output)) ensures false {",
    );
    let root = std::env::temp_dir().join(format!(
        "semaprax-byte-helper-failure-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("program.spx");
    std::fs::write(&path, &source).unwrap();
    for _ in 0..3 {
        let result = interpreter::interpret(
            &path,
            "app.main",
            &[],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        assert!(!result.returned);
        let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        assert_eq!(
            envelope["payload"]["outcome"]["status"]["domain_id"],
            "semaprax.contract.v1"
        );
        assert_eq!(envelope["payload"]["outcome"]["status"]["code"], 2);
    }
    crate::owned_vec_bytes_runtime::run_wasm_expected(&source, 10, 2, "none", 0);
    std::fs::remove_dir_all(root).unwrap();
}
