//! Explicit String leaf transfer; the original recursive reference still clones.
use super::*;
fn explicit() -> String {
    assert_eq!(RECURSIVE.matches("text:text").count(), 1);
    RECURSIVE.replace("text:text", "text:string_take(text)")
}
#[test]
fn recursive_explicit_string_transfer_matches_original_and_subtree_full_populations() {
    for populated in [false, true] {
        let take = explicit();
        for (label, pattern) in [
            ("original-clone", RECURSIVE),
            ("explicit-take", take.as_str()),
            ("whole-subtree", SUBTREE),
        ] {
            let root = directory(label);
            let text = source(pattern, populated);
            let path = root.join("app.spx");
            std::fs::write(&path, &text).unwrap();
            let ast = semaprax::check(&text, &path).unwrap();
            let canonical = semaprax::format::canonical(&ast);
            let again = semaprax::check(&canonical, &path).unwrap();
            assert_eq!(semaprax::format::canonical(&again), canonical);
            let graph = semaprax::graph::to_json(&ast).unwrap();
            semaprax::graph::verify_json(&ast, &graph).unwrap();
            let current = semaprax::hir::resolve(&ast).unwrap();
            semaprax::hir::validate(&current).unwrap();
            for _ in 0..3 {
                let execution = interpreter::internal_strings::interpret(
                    &path,
                    "app.main",
                    &[],
                    &InterpreterOptions::default(),
                )
                .unwrap();
                assert!(execution.returned, "{label}: {}", execution.envelope);
                interpreter::internal_strings::verify_envelope_against_source(
                    &execution.envelope,
                    &path,
                )
                .unwrap();
                let envelope: serde_json::Value =
                    serde_json::from_str(&execution.envelope).unwrap();
                assert_eq!(envelope["payload"]["outcome"]["value"], "42");
            }
            super::super::super::owned_collection_outcome::run_native(&ast, &root);
            super::super::super::owned_collection_outcome::run_strict_wasm(&ast, &root);
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
fn instrument(generated: &str, symbol: &str, probe: &str) -> String {
    let mut count = 0;
    let mut output = String::new();
    for line in generated.lines() {
        output.push_str(line);
        output.push('\n');
        if line.contains(&format!("spx_status_token {symbol}(")) && line.ends_with(" {") {
            count += 1;
            output.push_str(probe);
            output.push('\n');
        }
    }
    assert_eq!(count, 1, "exact current emitted entry {symbol}");
    output
}
fn field(id: &str) -> String {
    format!(
        "spx_field_{}",
        id.bytes().map(|b| format!("{b:02x}")).collect::<String>()
    )
}
#[test]
fn explicit_recursive_transfer_retains_pointer_without_allocating_copying_or_resizing() {
    for populated in [false, true] {
        let take = explicit();
        for (label, pattern, clone) in [
            ("original-clone", RECURSIVE, true),
            ("explicit-take", take.as_str(), false),
            ("whole-subtree", SUBTREE, false),
        ] {
            let root = directory(&format!("{label}-string-pointer"));
            let text = source(pattern, populated);
            let ast = semaprax::check(&text, root.join("app.spx")).unwrap();
            let generated = semaprax::codegen::emit_c(&ast)
                .unwrap()
                .replace("memcpy(", "take_memcpy(")
                .replace("memmove(", "take_memmove(")
                .replace("realloc(", "take_resize(")
                .replace(
                    "#define SPX_VEC_REALLOC realloc",
                    "#define SPX_VEC_REALLOC take_resize",
                );
            let generated = instrument(&generated, "spx_decl_666f7277617264", &format!(
                " ++take_forward; take_alloc_before=outcome_total_allocations; take_copy_before=take_copies; take_resize_before=take_resizes; take_pointer=spx_param_0->{}.{};", field("outer.report"), field("report.text")));
            let generated = instrument(&generated, "spx_decl_73636f7265", &format!(
                " ++take_score; if(outcome_total_allocations-take_alloc_before!={} || take_copies-take_copy_before!={} || take_resizes!=take_resize_before || (spx_param_0->{}==take_pointer)!={})abort();",
                u8::from(clone), u8::from(clone), field("report.text"), u8::from(!clone)));
            let probes = r#"#include <string.h>
static uint64_t take_alloc_before,take_copy_before,take_resize_before,take_copies,take_resizes,take_forward,take_score;
static const void *take_pointer;
static __attribute__((unused)) void *take_resize(void*p,size_t n){++take_resizes;return realloc(p,n);}
static __attribute__((unused)) void *take_memcpy(void*to,const void*from,size_t n){if(n)++take_copies;return memcpy(to,from,n);}
static __attribute__((unused)) void *take_memmove(void*to,const void*from,size_t n){if(n)++take_copies;return memmove(to,from,n);}
static __attribute__((destructor)) void take_done(void){if(take_forward!=3||take_score!=3)abort();}
"#;
            // Original owner construction is outside this native window. Real
            // O0/O2 wrappers preserve their complete once-only settlement census.
            super::super::super::owned_collection_outcome::run_native_generated(
                format!("{probes}{generated}"),
                &root,
            );
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
#[test]
fn explicit_recursive_transfer_keeps_pre_post_and_late_field_failure_status() {
    let take = explicit();
    for populated in [false, true] {
        for (label, replacement, domain, code, wasm_code) in [
            (
                "pre",
                "fn score(value:own Report)->i64 requires false {",
                "semaprax.contract.v1",
                1,
                "9",
            ),
            (
                "post",
                "fn score(value:own Report)->i64 ensures false {",
                "semaprax.contract.v1",
                2,
                "10",
            ),
            (
                "field",
                "fn score(value:own Report)->i64 {",
                "semaprax.arithmetic.v1",
                4,
                "4",
            ),
        ] {
            let mut text =
                source(&take, populated).replace("fn score(value:own Report)->i64 {", replacement);
            if label == "field" {
                assert_eq!(text.matches("metrics:Metrics{total:total}").count(), 1);
                text = text.replace("metrics:Metrics{total:total}", "metrics:Metrics{total:1/0}");
            }
            let root = directory(&format!("take-{label}"));
            let path = root.join("app.spx");
            std::fs::write(&path, &text).unwrap();
            let ast = semaprax::check(&text, &path).unwrap();
            for _ in 0..3 {
                let result = interpreter::internal_strings::interpret(
                    &path,
                    "app.main",
                    &[],
                    &InterpreterOptions::default(),
                )
                .unwrap();
                assert!(!result.returned);
                interpreter::internal_strings::verify_envelope_against_source(
                    &result.envelope,
                    &path,
                )
                .unwrap();
                let value: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
                let outcome = &value["payload"]["outcome"];
                assert_eq!(outcome["status"]["domain_id"], domain);
                assert_eq!(outcome["status"]["code"], code);
                assert!(outcome.get("value").is_none());
            }
            super::super::failure::native_failure(&ast, &root, domain, code);
            let bytes = semaprax::wasm::emit_module(&ast).unwrap();
            wasmparser::Validator::new().validate_all(&bytes).unwrap();
            let module = root.join("app.wasm");
            std::fs::write(&module, bytes).unwrap();
            let host = root.join("host.js");
            std::fs::write(&host, include_str!("../../owned_leaf_vec/host.js")).unwrap();
            let output = std::process::Command::new("node")
                .arg(host)
                .arg(module)
                .args([wasm_code, "0", label])
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{label}: {}",
                String::from_utf8_lossy(&output.stderr)
            );
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}
