//! Guard-return normalization is checked against independently authored value
//! control flow, including owners, contracts and a skipped checked failure.

use std::path::Path;
use std::process::Command;

use semaprax::{format, graph, hir, parse, verify};

const SOURCE: &str = r#"module test.guard_return;
@id("guard.choose")
fn choose(value: i64) -> i64
    ensures result >= 0
{
    let base = 3;
    if value < 0 { return base; }
    let doubled = value * 2;
    if doubled == 0 { return 7; }
    return doubled;
}
@id("guard.skip")
fn skip(flag: bool) -> i64 {
    if flag { return 11; }
    let failed = 1 / 0;
    failed
}
@id("guard.owned")
fn owned(flag: bool, text: string) -> string {
    let keep = "prefix";
    if flag { let local = "arm"; return string_concat(text, local); }
    let suffix = "tail";
    return string_concat(text, suffix);
}
@id("main")
fn main() -> i64 {
    let first = owned(true, "a");
    let second = owned(false, "b");
    choose(-5) + choose(0) + choose(9) + skip(true) + string_len(first) + string_len(second)
}
"#;

const LOWERED: &str = r#"module test.guard_return;
@id("guard.choose")
fn choose(value: i64) -> i64
    ensures result >= 0
{
    let base = 3;
    if value < 0 { base } else {
        let doubled = value * 2;
        if doubled == 0 { 7 } else { doubled }
    }
}
@id("guard.skip")
fn skip(flag: bool) -> i64 {
    if flag { 11 } else { let failed = 1 / 0; failed }
}
@id("guard.owned")
fn owned(flag: bool, text: string) -> string {
    let keep = "prefix";
    if flag { let local = "arm"; string_concat(text, local) } else {
        let suffix = "tail";
        string_concat(text, suffix)
    }
}
@id("main")
fn main() -> i64 {
    let first = owned(true, "a");
    let second = owned(false, "b");
    choose(-5) + choose(0) + choose(9) + skip(true) + string_len(first) + string_len(second)
}
"#;

#[test]
fn normalized_guards_preserve_graph_cleanup_and_canonical_source() {
    let path = Path::new("guard-return.spx");
    let source = parse(SOURCE, path).unwrap();
    let lowered = parse(LOWERED, path).unwrap();
    for program in [&source, &lowered] {
        assert!(
            verify::verify(program).is_empty(),
            "{:?}",
            verify::verify(program)
        );
        hir::validate(&hir::resolve(program).unwrap()).unwrap();
    }
    let canonical = format::canonical(&source);
    assert_eq!(canonical, format::canonical(&lowered));
    assert_eq!(
        canonical,
        format::canonical(&parse(&canonical, path).unwrap())
    );
    assert_eq!(graph::revision(&source), graph::revision(&lowered));
    assert_eq!(
        graph::to_json(&source).unwrap(),
        graph::to_json(&lowered).unwrap()
    );
    let actual = hir::resolve(&source).unwrap();
    let expected = hir::resolve(&lowered).unwrap();
    for (actual, expected) in actual.functions.iter().zip(&expected.functions) {
        assert_eq!(actual.cleanup_plan, expected.cleanup_plan);
        assert_eq!(actual.loan_plan, expected.loan_plan);
    }
}

#[test]
fn guard_returns_run_on_interpreter_native_and_core_wasm() {
    // Both branches exercise a consumed String and a branch-local owner.
    let source_path = super::write_source("guard-strings", SOURCE);
    let result = semaprax::interpreter::internal_strings::interpret(
        &source_path,
        "main",
        &[],
        &semaprax::interpreter::InterpreterOptions::default(),
    )
    .unwrap();
    let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
    assert_eq!(envelope["payload"]["outcome"]["kind"], "returned");
    assert_eq!(envelope["payload"]["outcome"]["value"], "48");
    std::fs::remove_dir_all(source_path.parent().unwrap()).unwrap();
    if super::command_available("clang") {
        assert_eq!(super::run(SOURCE, true), "48");
    }
    // The scalar subset must retain exactly the ordinary Core-Wasm bytes.
    let scalar = "module guard.scalar; @id(\"main\") fn main() -> i64 { if true { return 11; } let failed = 1 / 0; failed }";
    let ordinary = "module guard.scalar; @id(\"main\") fn main() -> i64 { if true { 11 } else { let failed = 1 / 0; failed } }";
    let path = Path::new("guard-scalar.spx");
    let bytes = semaprax::wasm::emit_module(&parse(scalar, path).unwrap()).unwrap();
    assert_eq!(
        bytes,
        semaprax::wasm::emit_module(&parse(ordinary, path).unwrap()).unwrap()
    );
    if super::command_available("node") {
        let source_path = super::write_source("guard-wasm", scalar);
        let directory = source_path.parent().unwrap();
        std::fs::write(directory.join("program.wasm"), bytes).unwrap();
        std::fs::write(directory.join("runner.mjs"), super::NODE_RUNNER).unwrap();
        let output = Command::new("node")
            .arg(directory.join("runner.mjs"))
            .arg(directory.join("program.wasm"))
            .output()
            .unwrap();
        std::fs::remove_dir_all(directory).unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&output.stdout).trim(), "11");
    }
}

#[test]
fn terminal_else_if_and_nested_guards_move_each_continuation_once() {
    for (body, expected) in [
        (
            "if true { return 5; } else { return 6; }",
            "if true { 5 } else { 6 }",
        ),
        (
            "if false { return 5; } else if false { return 6; } return 7;",
            "if false { 5 } else { if false { 6 } else { 7 } }",
        ),
        (
            "if true { if false { return 5; } return 6; } return 7;",
            "if true { if false { 5 } else { 6 } } else { 7 }",
        ),
    ] {
        let source = format!("module guard.branch; @id(\"main\") fn main() -> i64 {{ {body} }}");
        let expected =
            format!("module guard.branch; @id(\"main\") fn main() -> i64 {{ {expected} }}");
        let path = Path::new("guard-branch.spx");
        assert_eq!(
            format::canonical(&parse(&source, path).unwrap()),
            format::canonical(&parse(&expected, path).unwrap())
        );
    }
}

#[test]
fn returns_reject_unreachable_code_mixed_arms_and_unadmitted_contexts() {
    for (body, code) in [
        ("return 1; let later = 2; later", "SPX-P209"),
        ("if true { return 1; } else { 2 }", "SPX-P210"),
        ("if true { return 1; } else { return 2; } 3", "SPX-P209"),
        ("while true { return 1; } 0", "SPX-P106"),
        ("let value = { return 1; }; value", "SPX-P106"),
        ("let call = fn() -> i64 { return 1; }; call()", "SPX-P106"),
        ("while true { break; 0 } 0", "SPX-P106"),
        ("while true { continue; 0 } 0", "SPX-P106"),
    ] {
        let source = format!("module guard.refusal; @id(\"main\") fn main() -> i64 {{ {body} }}");
        let error = parse(&source, Path::new("guard-refusal.spx")).unwrap_err();
        assert_eq!(error.code, code, "{source}: {error}");
        assert!(error.help.is_some(), "{source}: {error}");
    }
}

#[test]
fn flat_guard_chain_is_bounded_before_recursive_materialization() {
    let mut source = String::from("module guard.bound; @id(\"main\") fn main() -> i64 {");
    for _ in 0..200 {
        source.push_str("if false { return 1; }");
    }
    source.push_str("return 2; }");
    assert_eq!(
        parse(&source, Path::new("guard-bound.spx"))
            .unwrap_err()
            .code,
        "SPX-P207"
    );
    let old = "module guard.ident; @id(\"main\") fn main() -> i64 { let return = 9; return + 1 }";
    assert!(parse(old, Path::new("guard-ident.spx")).is_ok());
}

#[test]
fn return_expression_failure_precedes_postcondition_failure() {
    let source = r#"module guard.failure;
@id("guard.fail") fn fail(flag: bool) -> i64 ensures result > 9 {
    let text = "live";
    if flag { return 1 / 0; }
    return string_len(text);
}
@id("main") fn main() -> i64 { fail(false) }
"#;
    let ordinary = source.replace(
        "if flag { return 1 / 0; }\n    return string_len(text);",
        "if flag { 1 / 0 } else { string_len(text) }",
    );
    let path = Path::new("guard-failure.spx");
    let program = parse(source, path).unwrap();
    let lowered = parse(&ordinary, path).unwrap();
    assert_eq!(
        graph::to_json(&program).unwrap(),
        graph::to_json(&lowered).unwrap()
    );
    assert_eq!(
        semaprax::codegen::emit_c(&program).unwrap(),
        semaprax::codegen::emit_c(&lowered).unwrap()
    );
    let source_path = super::write_source("guard-failures", source);
    for (flag, domain, code) in [
        ("true", "semaprax.arithmetic.v1", 4),
        ("false", "semaprax.contract.v1", 2),
    ] {
        let result = semaprax::interpreter::internal_strings::interpret(
            &source_path,
            "guard.fail",
            &[flag.to_owned()],
            &semaprax::interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
        let outcome = &envelope["payload"]["outcome"];
        assert_eq!(outcome["kind"], "failed");
        assert_eq!(outcome["status"]["domain_id"], domain);
        assert_eq!(outcome["status"]["code"], code);
    }
    std::fs::remove_dir_all(source_path.parent().unwrap()).unwrap();
}
