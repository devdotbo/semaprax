//! Same-source nested Vec owners, projected loans, and physical settlement.
use semaprax::interpreter::{self, InterpreterOptions};
use std::sync::atomic::{AtomicU64, Ordering};
#[path = "nested_collection_record/failure.rs"]
pub(super) mod failure;
mod subtree;

static NEXT: AtomicU64 = AtomicU64::new(0);
const ROWS: &str = include_str!("../fixtures/nested-collection-records.spx");
const STRINGS: &str = include_str!("../fixtures/nested-collection-string-record.spx");
fn directory(label: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!(
        "nested-collection-{}-{label}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn nested_collection_record_borrow_forward_and_destructure_settle_on_three_backends() {
    for (label, source) in [("rows", ROWS), ("strings", STRINGS)] {
        let root = directory(label);
        let path = root.join("app.spx");
        std::fs::write(&path, source).unwrap();
        let ast = semaprax::check(source, &path).unwrap();
        for _ in 0..3 {
            let result =
                interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default())
                    .unwrap();
            interpreter::verify_envelope(&result.envelope).unwrap();
            let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            assert!(result.returned, "{}", result.envelope);
            assert_eq!(envelope["payload"]["outcome"]["value"], "42");
        }
        super::owned_collection_outcome::run_native(&ast, &root);
        super::owned_collection_outcome::run_strict_wasm(&ast, &root);
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn nested_collection_empty_vec_and_reordered_deep_siblings_keep_independent_owners() {
    let source = STRINGS.replace("@id(\"app.main\")", r#"
@id("collection.envelope") record Envelope {
 @id("collection.envelope.payload") payload:Bytes,
 @id("collection.envelope.report") report:Report,
 @id("collection.envelope.label") label:string,
}
@id("collection.deep") fn deep(value:own Envelope)->i64 {
 let count=i64_from_usize(vec_len<string>(value.report.items));
 match own value {Envelope{payload,report:Report{items,metrics:Metrics{selected}},label}=>
  count+i64_from_usize(vec_len<string>(items))+selected+string_len(label)+i64_from_usize(byte_len(bytes_as_slice(payload))),}
}
@id("app.main")"#);
    let prefix = source.split("@id(\"app.main\")").next().unwrap();
    let source = format!(
        "{prefix}@id(\"app.main\") fn main()->i64 {{let data=Envelope{{payload:bytes_zeroed(3usize),report:Report{{items:vec_with_capacity<string>(0usize),metrics:Metrics{{selected:35}}}},label:\"abcd\"}};deep(data)}}"
    );
    let root = directory("deep-empty");
    let path = root.join("app.spx");
    std::fs::write(&path, &source).unwrap();
    let ast = semaprax::check(&source, &path).unwrap();
    let result =
        interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
    assert!(result.returned, "{}", result.envelope);
    let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    assert_eq!(envelope["payload"]["outcome"]["value"], "42");
    super::owned_collection_outcome::run_native(&ast, &root);
    super::owned_collection_outcome::run_strict_wasm(&ast, &root);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn nested_collection_owned_match_result_and_terminal_return_settle_on_three_backends() {
    let source = include_str!("../fixtures/nested-collection-match-results.spx");
    let root = directory("match-result");
    let path = root.join("app.spx");
    std::fs::write(&path, source).unwrap();
    let ast = semaprax::check(source, &path).unwrap();
    for _ in 0..3 {
        let result =
            interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
        interpreter::verify_envelope(&result.envelope).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        assert!(result.returned, "{}", result.envelope);
        assert_eq!(envelope["payload"]["outcome"]["value"], "42");
    }
    super::owned_collection_outcome::run_native(&ast, &root);
    super::owned_collection_outcome::run_strict_wasm(&ast, &root);
    let failing = source.replace(
        "fn terminal(value:own Report, early:bool)->Wrapped {",
        "fn terminal(value:own Report, early:bool)->Wrapped ensures false {",
    );
    assert_ne!(failing, source);
    std::fs::write(&path, &failing).unwrap();
    let ast = semaprax::check(&failing, &path).unwrap();
    for _ in 0..3 {
        let result =
            interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
        interpreter::verify_envelope(&result.envelope).unwrap();
        assert!(!result.returned, "{}", result.envelope);
        assert!(result
            .envelope
            .contains("\"domain_id\":\"semaprax.contract.v1\""));
        assert!(result.envelope.contains("\"code\":2"));
    }
    failure::native_failure(&ast, &root, "semaprax.contract.v1", 2);
    let module = root.join("failure.wasm");
    std::fs::write(&module, semaprax::wasm::emit_module(&ast).unwrap()).unwrap();
    let host = root.join("host.js");
    std::fs::write(&host, include_str!("owned_leaf_vec/host.js")).unwrap();
    let output = std::process::Command::new("node")
        .arg(host)
        .arg(module)
        .args(["10", "0", "prefix-failure"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}
