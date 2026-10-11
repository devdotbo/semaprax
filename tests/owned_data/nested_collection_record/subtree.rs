//! Exact subtree movement versus independent recursive unpack/rebuild.
use super::*;
const PREFIX: &str = r#"module test.subtree;
@id("metrics") record Metrics {@id("metrics.total") total:i64,}
@id("report") record Report {
 @id("report.bytes") bytes:Bytes,
 @id("report.text") text:string,
 @id("report.words") words:Vec<string>,
 @id("report.metrics") metrics:Metrics,
}
@id("sibling") record Sibling {@id("sibling.bytes") bytes:Bytes,@id("sibling.tag") tag:i64,}
@id("outer") record Outer {@id("outer.valid") valid:bool,@id("outer.report") report:Report,@id("outer.sibling") sibling:Sibling,}
@id("score") fn score(value:own Report)->i64 {
 match own value {Report{bytes,text,words,metrics:Metrics{total}}=>
 i64_from_usize(byte_len(bytes_as_slice(bytes)))+string_len(text)+i64_from_usize(vec_len<string>(words))+total,}
}
"#;
const SUBTREE: &str = r#"@id("forward") fn forward(value:own Outer)->i64 {
 let answer=match own value {Outer{sibling:Sibling{bytes:unused,tag:_},report,valid}=>
 if valid {score(report)}else{0},};
 let mut once=true;
 while once {once=false;if answer==42 {return answer;}else{0};}
 answer
}
"#;
const RECURSIVE: &str = r#"@id("forward") fn forward(value:own Outer)->i64 {
 let answer=match own value {Outer{sibling:Sibling{bytes:unused,tag:_},report:Report{bytes,text,words,metrics:Metrics{total}},valid}=>
 if valid {score(Report{bytes:bytes,text:text,words:words,metrics:Metrics{total:total}})}else{0},};
 let mut once=true;
 while once {once=false;if answer==42 {return answer;}else{0};}
 answer
}
"#;
fn source(pattern: &str, populated: bool) -> String {
    let words = if populated {
        "vec_push<string>(vec_push<string>(vec_with_capacity<string>(2usize),\"a\"),\"b\")"
    } else {
        "vec_with_capacity<string>(0usize)"
    };
    let total = if populated { 33 } else { 35 };
    format!("{PREFIX}{pattern}@id(\"app.main\") fn main()->i64 {{let outer=Outer{{valid:true,report:Report{{bytes:bytes_zeroed(3usize),text:\"four\",words:{words},metrics:Metrics{{total:{total}}}}},sibling:Sibling{{bytes:bytes_zeroed(1usize),tag:9}}}};forward(outer)}}")
}

#[test]
fn whole_subtree_and_recursive_control_settle_empty_and_full_vec_on_three_backends() {
    for populated in [false, true] {
        let mut oracle = None;
        for (label, pattern) in [("recursive", RECURSIVE), ("subtree", SUBTREE)] {
            let root = directory(label);
            let path = root.join("app.spx");
            let text = source(pattern, populated);
            std::fs::write(&path, &text).unwrap();
            let ast = semaprax::check(&text, &path).unwrap();
            let canonical = semaprax::format::canonical(&ast);
            assert_eq!(
                semaprax::format::canonical(&semaprax::check(&canonical, &path).unwrap()),
                canonical
            );
            let hir = semaprax::hir::resolve(&ast).unwrap();
            semaprax::hir::validate(&hir).unwrap();
            let function = hir
                .functions
                .iter()
                .find(|f| f.id.as_str() == "forward")
                .unwrap();
            assert!(function.cleanup_plan.lexical_control.is_some());
            for _ in 0..3 {
                let execution =
                    interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default())
                        .unwrap();
                interpreter::verify_envelope(&execution.envelope).unwrap();
                let value: serde_json::Value = serde_json::from_str(&execution.envelope).unwrap();
                assert!(execution.returned, "{label}: {}", execution.envelope);
                assert_eq!(value["payload"]["outcome"]["value"], "42");
                if let Some(expected) = &oracle {
                    assert_eq!(&value["payload"]["outcome"], expected);
                } else {
                    oracle = Some(value["payload"]["outcome"].clone());
                }
            }
            super::super::owned_collection_outcome::run_native(&ast, &root);
            super::super::owned_collection_outcome::run_strict_wasm(&ast, &root);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
    // A failed postcondition after the subtree transfer must settle the child
    // and its siblings without publishing the computed scalar result.
    for (label, pattern) in [
        ("recursive-contract", RECURSIVE),
        ("subtree-contract", SUBTREE),
    ] {
        let root = directory(label);
        let path = root.join("app.spx");
        let text = source(pattern, true).replace(
            "fn score(value:own Report)->i64 {",
            "fn score(value:own Report)->i64 ensures false {",
        );
        std::fs::write(&path, &text).unwrap();
        let ast = semaprax::check(&text, &path).unwrap();
        for _ in 0..3 {
            let execution =
                interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default())
                    .unwrap();
            assert!(!execution.returned, "{label}: {}", execution.envelope);
            interpreter::verify_envelope(&execution.envelope).unwrap();
            assert!(execution
                .envelope
                .contains("\"domain_id\":\"semaprax.contract.v1\""));
            assert!(execution.envelope.contains("\"code\":2"));
        }
        super::failure::native_failure(&ast, &root, "semaprax.contract.v1", 2);
        let bytes = semaprax::wasm::emit_module(&ast).unwrap();
        wasmparser::Validator::new().validate_all(&bytes).unwrap();
        let module = root.join("failure.wasm");
        std::fs::write(&module, bytes).unwrap();
        let host = root.join("host.js");
        std::fs::write(&host, include_str!("../owned_leaf_vec/host.js")).unwrap();
        let result = std::process::Command::new("node")
            .arg(host)
            .arg(module)
            .args(["10", "0", "subtree-contract"])
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{label}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}

#[test]
fn subtree_transfer_refuses_hidden_owner_borrow_use_after_move_and_live_descendant() {
    let valid = source(SUBTREE, true);
    for (label, from, to, code) in [
        ("wildcard", "report,valid", "report:_,valid", "SPX-O117"),
        (
            "borrow",
            "match own value {Outer",
            "match borrow value {Outer",
            "SPX-O117",
        ),
        (
            "moved",
            "score(report)",
            "score(report)+score(report)",
            "SPX-O101",
        ),
        (
            "live",
            "let answer=match own value",
            "let view=bytes_as_slice(value.report.bytes);let answer=match own value",
            "SPX-T265",
        ),
    ] {
        let changed = valid.replace(from, to);
        assert_ne!(changed, valid);
        // Use after the match keeps the descendant loan live at the root move.
        let changed = if label == "live" {
            changed.replace(" answer\n}", " answer+i64_from_usize(byte_len(view))\n}")
        } else {
            changed
        };
        let errors = semaprax::check(&changed, "subtree-hostile.spx").unwrap_err();
        assert!(errors.iter().any(|d| d.code == code), "{label}: {errors:?}");
    }
}
