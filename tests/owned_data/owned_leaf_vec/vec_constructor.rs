//! Same ordinary calls, one owning result, and canonical settlement on reentry.
use super::{DECLARATION, HOST, SEQUENCE};
use std::sync::atomic::Ordering;

fn body(count: usize) -> String {
    let items = (0..count)
        .map(|n| format!("Row{{priority:{n},title:\"é\\u{{0}}\",payload:bytes_zeroed(3usize)}}"))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        r#"@id("app.main") fn main()->i64 {{
 let rows=vec_of<Row>({items});
 let mut good=true; let mut seen=0usize;
 for borrow row at index in rows {{
  let text=row.title; let data=row.payload;
  good=good && row.priority==i64_from_usize(index) && str_len_bytes(text)==3 && byte_len(data)==3usize;
  seen=seen+1usize; 0
 }}
 if good && seen=={count}usize && vec_len<Row>(rows)=={count}usize {{42}}else{{-1}}
}}"#
    )
}

fn wasm(source: &semaprax::ast::Program, root: &std::path::Path, status: u32, mode: &str) {
    let path = root.join("app.wasm");
    std::fs::write(&path, semaprax::wasm::emit_module(source).unwrap()).unwrap();
    let output = std::process::Command::new("node")
        .arg(HOST)
        .arg(path)
        .arg(status.to_string())
        .arg(if status == 0 { "42" } else { "0" })
        .arg(mode)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{mode}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn constructors_execute_zero_one_eight_and_byte_event_boundary_rows_on_all_engines() {
    for count in [0, 1, 8, 32] {
        let root = std::env::temp_dir().join(format!(
            "vec-constructor-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("app.spx");
        let source = format!("{DECLARATION}{}", body(count));
        std::fs::write(&path, &source).unwrap();
        let ast = semaprax::check(&source, &path).unwrap();
        let resolved = semaprax::hir::resolve(&ast).unwrap();
        semaprax::hir::validate(&resolved).unwrap();
        let graph = semaprax::graph::to_json(&ast).unwrap();
        semaprax::graph::verify_json(&ast, &graph).unwrap();
        for _ in 0..3 {
            let result =
                semaprax::interpreter::interpret(&path, "app.main", &[], &Default::default())
                    .unwrap();
            assert!(result.returned, "{}", result.envelope);
            assert!(
                result.envelope.contains("\"value\":\"42\""),
                "{}",
                result.envelope
            );
        }
        super::super::owned_collection_outcome::run_native_generated_with_allocation_count(
            semaprax::codegen::emit_c(&ast).unwrap(),
            &root,
            None,
        );
        wasm(&ast, &root, 0, "none");
        if count == 8 {
            for mode in ["codec-push-1", "codec-push-4"] {
                wasm(&ast, &root, 13, mode);
            }
        }
        if count == 8 {
            let failed_source =
                source.replace("fn main()->i64 {", "fn main()->i64 ensures false {");
            let failed = semaprax::check(&failed_source, &path).unwrap();
            std::fs::write(&path, &failed_source).unwrap();
            for _ in 0..3 {
                let result =
                    semaprax::interpreter::interpret(&path, "app.main", &[], &Default::default())
                        .unwrap();
                assert!(!result.returned, "{}", result.envelope);
                let wire: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
                assert_eq!(
                    wire["payload"]["outcome"]["status"]["domain_id"],
                    "semaprax.contract.v1"
                );
                assert_eq!(wire["payload"]["outcome"]["status"]["code"], 2);
            }
            super::super::nested_collection_record::failure::native_failure(
                &failed,
                &root,
                "semaprax.contract.v1",
                2,
            );
            wasm(&failed, &root, 10, "none");
            let partial_source = source
                .replace("@id(\"app.main\")", "@id(\"constructor.boom\") fn boom()->i64{9223372036854775807+1} @id(\"app.main\")")
                .replace("priority:1,", "priority:boom(),");
            let partial = semaprax::check(&partial_source, &path).unwrap();
            std::fs::write(&path, &partial_source).unwrap();
            for _ in 0..3 {
                let result =
                    semaprax::interpreter::interpret(&path, "app.main", &[], &Default::default())
                        .unwrap();
                assert!(!result.returned, "{}", result.envelope);
                let wire: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
                assert_eq!(
                    wire["payload"]["outcome"]["status"]["domain_id"],
                    "semaprax.arithmetic.v1"
                );
                assert_eq!(wire["payload"]["outcome"]["status"]["code"], 1);
            }
            super::super::nested_collection_record::failure::native_failure(
                &partial,
                &root,
                "semaprax.arithmetic.v1",
                1,
            );
            wasm(&partial, &root, 1, "none");
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[path = "vec_constructor_failure.rs"]
mod failure;
