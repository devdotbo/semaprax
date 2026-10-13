//! Authored, pending execution: exact application helper versus its frozen
//! indexed helper and a host oracle. Core fixtures do not claim a Wasm command
//! adapter for the native-only Catalog Project.
use super::{raw_wasm, Command, Path};
use semaprax::{codegen, format, graph, hir, interpreter};

const ORDER: &str = include_str!("../../../examples/catalog-scoped-record-successor/src/order.spx");
const OLD_ORDER: &str = include_str!(
    "../../../docs/evidence/application-traversal-plan-adoption/order_before_traversal.txt"
);

fn helper(source: &str) -> &str {
    &source[source.find("@id(\"catalog.identifier\")").unwrap()
        ..source.find("@id(\"catalog.valid_sorted_tags\")").unwrap()]
}

fn accepted(bytes: &[u8]) -> bool {
    (1..=16).contains(&bytes.len())
        && bytes
            .iter()
            .all(|b| b.is_ascii_alphanumeric() || matches!(*b, b'_' | b'-'))
}

fn literal(value: &str) -> String {
    let mut result = String::from("\"");
    for c in value.chars() {
        if c.is_ascii_alphanumeric() || matches!(c, '_' | '-') {
            result.push(c);
        } else {
            result.push_str(&format!("\\u{{{:x}}}", u32::from(c)));
        }
    }
    result.push('"');
    result
}

fn assert_engines(root: &Path, name: &str, source: &str, expected: i64, string_bytes: usize) {
    let path = root.join(format!("{name}.spx"));
    let parsed = semaprax::check(source, &path).unwrap_or_else(|e| panic!("{name}: {e:?}"));
    let canonical = format::canonical(&parsed);
    let parsed = semaprax::check(&canonical, &path).unwrap();
    assert_eq!(format::canonical(&parsed), canonical);
    hir::validate(&hir::resolve(&parsed).unwrap()).unwrap();
    let projection = graph::to_json(&parsed).unwrap();
    graph::verify_json(&parsed, &projection).unwrap();
    let wire: serde_json::Value = serde_json::from_str(&projection).unwrap();
    assert_eq!(wire["schema"], "semaprax.graph.v83");
    std::fs::write(&path, canonical).unwrap();
    let outcome = interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
    assert!(outcome.returned, "{name}: {}", outcome.envelope);
    interpreter::verify_envelope(&outcome.envelope).unwrap();
    let wire: serde_json::Value = serde_json::from_str(&outcome.envelope).unwrap();
    assert_eq!(wire["payload"]["outcome"]["value"], expected.to_string());
    let c = root.join(format!("{name}.c"));
    std::fs::write(&c, codegen::emit_c(&parsed).unwrap()).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("{name}-{optimization}"));
        let compiled = Command::new("clang")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
            .arg(&c)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = Command::new(binary).output().unwrap();
        assert!(
            output.status.success(),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            String::from_utf8_lossy(&output.stdout).trim(),
            expected.to_string()
        );
    }
    let module = semaprax::wasm::emit_module(&parsed).unwrap();
    assert_eq!(module, semaprax::wasm::emit_module(&parsed).unwrap());
    // The owned String literal uses one Bytes-ABI allocation; the borrowed
    // traversal adds none. Keep that setup allocation explicit in the oracle.
    raw_wasm(root, &format!("catalog-{string_bytes}"), &module, expected);
}

#[test]
fn catalog_identifier_matches_frozen_helper_and_byte_oracle_on_every_engine() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-catalog-byte-helper-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    let old = helper(OLD_ORDER)
        .replace("catalog.identifier", "catalog.original_identifier")
        .replace("fn identifier(", "fn original_identifier(");
    assert!(old.contains("while valid && index < length"));
    assert!(!old.contains("for byte"));
    for (index, value) in [
        "",
        "A",
        "z",
        "0",
        "_",
        "-",
        "AZaz09_-",
        "0123456789ABCDEF",
        "0123456789ABCDEFG",
        "@ab",
        "a@b",
        "ab@",
        "\0",
        "a\0b",
        "é",
        "e\u{301}",
        "🦀",
        "AéZ",
    ]
    .into_iter()
    .enumerate()
    {
        let expected = accepted(value.as_bytes());
        let source = format!("module app.catalog_identifier;\n{}\n{old}\n@id(\"app.main\") fn main()->i64 {{ let value={}; let before=original_identifier(string_as_str(value)); let after=identifier(string_as_str(value)); if before=={expected} && after=={expected} {{0}}else{{1}} }}", helper(ORDER), literal(value));
        assert_engines(&root, &format!("case-{index}"), &source, 0, value.len());
    }
    std::fs::remove_dir_all(root).unwrap();
}

// Test-only observation: fold each visited byte before testing the production
// predicate. Returning the fold makes the complete visited prefix observable;
// a later '!' invokes a failing precondition if traversal ever reaches it.
const PREFIX: &str = r#"
@id("catalog.prefix.observe")
fn observe(byte:u8)->i64 requires byte != 33u8 { i64_from_u8(byte) }
@id("catalog.prefix")
fn prefix(value:borrow str)->i64 {
    let bytes=str_as_bytes(value);
    let length=byte_len(bytes);
    if length==0usize || length>16usize {return 0;};
    let mut seen=0;
    for byte in bytes {
        seen=(seen*131+observe(byte))%1000000007;
        if !(byte>=65u8 && byte<=90u8 || byte>=97u8 && byte<=122u8 || byte>=48u8 && byte<=57u8 || byte==95u8 || byte==45u8) {return seen;};
        0
    }
    seen
}
"#;

#[test]
fn catalog_identifier_early_stop_observes_exact_prefix_and_skips_failure_sentinel() {
    let root = std::env::temp_dir().join(format!(
        "semaprax-catalog-byte-prefix-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).unwrap();
    // Bounds skip all observation; invalid first/middle/last bytes terminate in
    // order. The sentinel is invalid too and would fail if eagerly observed.
    for (index, value) in [
        "",
        "!!!!!!!!!!!!!!!!!",
        "@!",
        "A@!",
        "AB@!",
        "AB@",
        "é!",
        "AZaz09_-",
        "AAAAAAAAAAAAAAAA",
    ]
    .into_iter()
    .enumerate()
    {
        let bytes = value.as_bytes();
        let mut expected = 0;
        if (1..=16).contains(&bytes.len()) {
            for byte in bytes {
                assert_ne!(*byte, b'!', "host prefix unexpectedly reached sentinel");
                expected = (expected * 131 + i64::from(*byte)) % 1_000_000_007;
                if !byte.is_ascii_alphanumeric() && !matches!(*byte, b'_' | b'-') {
                    break;
                }
            }
        }
        let source = format!("module app.catalog_prefix;\n{PREFIX}\n@id(\"app.main\") fn main()->i64 {{let value={}; prefix(string_as_str(value))}}", literal(value));
        assert_engines(
            &root,
            &format!("prefix-{index}"),
            &source,
            expected,
            value.len(),
        );
    }
    // Counterexample: put the sentinel first so the observation precondition
    // really fails on every engine, rather than trusting an inert marker.
    let source = format!("module app.catalog_prefix_failure;\n{PREFIX}\n@id(\"app.main\") fn main()->i64 {{let value=\"!\"; prefix(string_as_str(value))}}");
    let path = root.join("sentinel-reached.spx");
    let parsed = semaprax::check(&source, &path).unwrap();
    hir::validate(&hir::resolve(&parsed).unwrap()).unwrap();
    std::fs::write(&path, &source).unwrap();
    let outcome = interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
    assert!(!outcome.returned);
    interpreter::verify_envelope(&outcome.envelope).unwrap();
    let wire: serde_json::Value = serde_json::from_str(&outcome.envelope).unwrap();
    assert_eq!(
        wire["payload"]["outcome"]["status"]["domain_id"],
        "semaprax.contract.v1"
    );
    assert_eq!(wire["payload"]["outcome"]["status"]["code"], 1);
    let c = root.join("sentinel-reached.c");
    std::fs::write(&c, codegen::emit_c(&parsed).unwrap()).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("sentinel-reached-{optimization}"));
        let compiled = Command::new("clang")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
            .arg(&c)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let output = Command::new(binary).output().unwrap();
        assert_eq!(output.status.code(), Some(70));
        assert!(String::from_utf8_lossy(&output.stderr)
            .contains("requires byte != 33u8 in catalog.prefix.observe"));
    }
    let module = semaprax::wasm::emit_module(&parsed).unwrap();
    raw_wasm(&root, "catalog-failure-1", &module, 0);
    std::fs::remove_dir_all(root).unwrap();
}
