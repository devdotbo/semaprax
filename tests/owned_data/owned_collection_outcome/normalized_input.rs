//! Exact ordinary Input→existing outcome joins, with the old callee workaround.
use semaprax::{codegen, interpreter, wasm};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
const SOURCE: &str = include_str!("../../fixtures/normalized-input-flat-outcome.spx");
static NEXT: AtomicU64 = AtomicU64::new(0);
fn source(copy: bool, failure: u32) -> String {
    let prefix = SOURCE.split("@id(\"app.main\")").next().unwrap();
    let body = if failure == 2 {
        "let unpublished=guard(Input::Ready{bytes:bytes_zeroed(2usize),length:2usize});0"
    } else if failure == 1 {
        "let unpublished=join(Input::Ready{bytes:bytes_zeroed(2usize),length:2usize});0"
    } else {
        "let empty=decide(join(Input::Ready{bytes:bytes_zeroed(0usize),length:0usize}));
         let full=decide(join(Input::Ready{bytes:bytes_zeroed(2usize),length:2usize}));
         let error=decide(join(Input::Error{code:42,offset:2usize,field:3}));
         let decoded_error=decide(join(Input::Ready{bytes:bytes_zeroed(3usize),length:3usize}));
         let old_empty=decide(receive(bytes_zeroed(0usize),0usize));
         let old_full=decide(receive(bytes_zeroed(2usize),2usize));
         if empty==42 && full==42 && error==42 && decoded_error==42 && old_empty==empty && old_full==full {42}else{0}"
    };
    let mut source = format!(
        "{prefix}@id(\"guard\") fn guard(input:own Input)->Output ensures false {{join(input)}}\n@id(\"app.main\") fn main()->i64 {{{body}}}"
    );
    if failure == 1 {
        source = source.replace(
            "vec_with_capacity<Row>(1usize)",
            "vec_with_capacity<Row>(0usize)",
        );
    }
    if copy {
        source = source
            .replace("label: string", "label: bool")
            .replace("label: \"ok\"", "label: true")
            .replace("label == \"ok\"", "label")
            .replace("match own first", "match first")
            .replace(
                "vec_clone_at<Row>(rows, 0usize)",
                "vec_get<Row>(rows, 0usize)",
            );
    }
    source
}
fn run(copy: bool, failure: u32) {
    let source = source(copy, failure);
    let root = std::env::temp_dir().join(format!(
        "normalized-flat-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("app.spx");
    std::fs::write(&path, &source).unwrap();
    let ast = semaprax::check(&source, &path).unwrap();
    for _ in 0..3 {
        let result = interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
        interpreter::verify_envelope(&result.envelope).unwrap();
        let wire: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        if failure != 0 {
            assert!(!result.returned, "{}", result.envelope);
            assert_eq!(
                wire["payload"]["outcome"]["status"]["domain_id"],
                if failure == 2 {
                    "semaprax.contract.v1"
                } else {
                    "semaprax.vec.v1"
                }
            );
            assert_eq!(
                wire["payload"]["outcome"]["status"]["code"],
                if failure == 2 { 2 } else { 1 }
            );
        } else {
            assert!(result.returned, "{}", result.envelope);
            assert_eq!(wire["payload"]["outcome"]["value"], "42");
        }
    }
    if failure != 0 {
        super::super::nested_collection_record::failure::native_failure(
            &ast,
            &root,
            if failure == 2 {
                "semaprax.contract.v1"
            } else {
                "semaprax.vec.v1"
            },
            if failure == 2 { 2 } else { 1 },
        );
    } else {
        super::run_native(&ast, &root);
    }
    let module = root.join("app.wasm");
    std::fs::write(&module, wasm::emit_module(&ast).unwrap()).unwrap();
    let host = root.join("host.js");
    std::fs::write(&host, include_str!("../owned_leaf_vec/host.js")).unwrap();
    // The existing strict Copy boundary uses its exact legacy scalar imports;
    // do not manufacture an owned-leaf provider for the Copy family.
    std::fs::write(
        root.join("copy_scalar_imports.js"),
        include_str!("../owned_leaf_vec/copy_scalar_imports.js"),
    )
    .unwrap();
    let mode = if copy {
        "copy-record-codec"
    } else {
        "prefix-failure"
    };
    let output = Command::new("node")
        .arg(&host)
        .arg(&module)
        .args([
            if failure == 2 {
                "10"
            } else if failure == 1 {
                "13"
            } else {
                "0"
            },
            if failure != 0 { "0" } else { "42" },
            mode,
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "copy={copy} failure={failure}: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let _ = codegen::emit_c(&ast).unwrap();
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn both_flat_normalizer_joins_equal_workaround_and_settle_all_owners_on_three_backends() {
    for copy in [false, true] {
        run(copy, 0);
    }
}
#[test]
fn joined_callee_and_postcondition_refusal_settles_input_and_output_without_publication_on_reentry()
{
    for copy in [false, true] {
        run(copy, 1);
        run(copy, 2);
    }
}
