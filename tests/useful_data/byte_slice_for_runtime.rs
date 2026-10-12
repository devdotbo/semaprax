//! Immutable borrowed `Slice<u8>` traversal against an independent manual
//! `byte_len`/`byte_get` loop and a Rust oracle on the interpreter, native
//! C11 `-O0`/`-O2` and Core-Wasm: empty, singleton, all 256 bytes, a
//! subrange, an owned buffer and a String root; `continue`, `break`, `return`,
//! lazy operands and a body failure that keeps its selected status.
use std::path::Path;
use std::process::Command;

use semaprax::{codegen, hir, interpreter, parse};

const SOURCE: &str = include_str!("../fixtures/byte-slice-traversal.spx");

const FAILURE: &str = r#"module app.byte_traversal_failure;

@id("app.failure.reject")
fn reject(byte: u8) -> i64
    requires byte != 9u8
{
    i64_from_u8(byte)
}

@id("app.main")
fn main() -> i64 {
    let data = [1u8, 9u8, 2u8];
    let owned = bytes_copy(array_as_slice(data));
    let view = bytes_as_slice(owned);
    let mut total = 0;
    for byte in view { total = total + reject(byte); 0 }
    total
}
"#;

/// A traversal-only module: parameter and array views, no owned buffer.
const VIEW_ONLY: &str = r#"module app.byte_traversal_view_only;

@id("app.view.sum")
fn sum(view: borrow Slice<u8>) -> i64 {
    let mut total = 0;
    for byte in view { total = total + i64_from_u8(byte); 0 }
    total
}

@id("app.main")
fn main() -> i64 {
    let data = [1u8, 2u8, 3u8];
    sum(array_as_slice(data))
}
"#;

fn mix(total: i64, byte: u8) -> i64 {
    (total * 31 + i64::from(byte)) % 1_000_000_007
}

fn fold(bytes: &[u8]) -> i64 {
    bytes.iter().fold(0, |total, byte| mix(total, *byte))
}

fn expected() -> i64 {
    let all = (0..=255u8).collect::<Vec<_>>();
    let cases: [&[u8]; 6] = [&[], &[7], &all, &all[10..20], &all, b"SEMAPRAX"];
    let whole = cases.iter().map(|bytes| fold(bytes)).sum::<i64>();
    let mut early = 0;
    for &byte in &all {
        if byte == 3 {
            continue;
        }
        if byte == 250 {
            break;
        }
        early = mix(early, byte);
    }
    let located = 200 + -1;
    let lazy = 1;
    whole * 2 + early * 2 + located + lazy + fold(&[6, 7])
}

#[test]
fn byte_for_matches_manual_loop_and_oracle_on_every_engine() {
    let expected = expected();
    let program = parse(SOURCE, Path::new("byte-slice-traversal.spx")).unwrap();
    hir::validate(&hir::resolve(&program).unwrap()).unwrap();
    let failure = parse(FAILURE, Path::new("byte-slice-traversal-failure.spx")).unwrap();
    hir::validate(&hir::resolve(&failure).unwrap()).unwrap();

    let root = std::env::temp_dir().join(format!("semaprax-byte-slice-for-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&root);
    std::fs::create_dir_all(&root).unwrap();
    let success_source = root.join("success.spx");
    let failure_source = root.join("failure.spx");
    std::fs::write(&success_source, SOURCE).unwrap();
    std::fs::write(&failure_source, FAILURE).unwrap();

    for _ in 0..4 {
        let outcome = interpreter::interpret(
            &success_source,
            "app.main",
            &[],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        assert!(outcome.returned, "{}", outcome.envelope);
        assert!(
            outcome
                .envelope
                .contains(&format!("\"value\":\"{expected}\"")),
            "{}",
            outcome.envelope
        );
        let failed = interpreter::interpret(
            &failure_source,
            "app.main",
            &[],
            &interpreter::InterpreterOptions::default(),
        )
        .unwrap();
        assert!(!failed.returned);
        let envelope: serde_json::Value = serde_json::from_str(&failed.envelope).unwrap();
        assert_eq!(
            envelope["payload"]["outcome"]["status"]["domain_id"],
            "semaprax.contract.v1"
        );
        assert_eq!(envelope["payload"]["outcome"]["status"]["code"], 1);
    }

    let success_c = root.join("success.c");
    let failure_c = root.join("failure.c");
    std::fs::write(&success_c, codegen::emit_c(&program).unwrap()).unwrap();
    std::fs::write(&failure_c, codegen::emit_c(&failure).unwrap()).unwrap();
    for optimization in ["-O0", "-O2"] {
        for (source, binary) in [(&success_c, "success"), (&failure_c, "failure")] {
            let binary = root.join(format!("{binary}-{optimization}"));
            let compiled = Command::new("clang")
                .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
                .arg(source)
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "{optimization}: {}",
                String::from_utf8_lossy(&compiled.stderr)
            );
        }
        for _ in 0..4 {
            let output = Command::new(root.join(format!("success-{optimization}")))
                .output()
                .unwrap();
            assert!(output.status.success(), "success/{optimization}");
            assert_eq!(
                String::from_utf8_lossy(&output.stdout).trim(),
                expected.to_string()
            );
            let output = Command::new(root.join(format!("failure-{optimization}")))
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(70), "failure/{optimization}");
            assert!(String::from_utf8_lossy(&output.stderr)
                .contains("requires byte != 9u8 in app.failure.reject"));
        }
    }

    if Command::new("node").arg("--version").output().is_ok() {
        for (name, program, outcome) in [
            (
                "success",
                &program,
                format!("if(value!=={expected}n)throw Error('semantic:'+value);"),
            ),
            (
                "failure",
                &failure,
                "throw Error('body failure returned:'+value);".to_owned(),
            ),
        ] {
            let web = root.join(format!("web-{name}"));
            semaprax::wasm::build_web(program, &web).unwrap();
            std::fs::write(web.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
            std::fs::write(
                web.join("probe.mjs"),
                format!(
                    "import {{readFile}} from 'node:fs/promises';\nimport {{instantiateBytes}} from './semaprax.js';\nconst bytes=await readFile('./app.wasm');\nfor(let i=0;i<4;i+=1){{const {{instance}}=await instantiateBytes(bytes);let value;try{{value=instance.exports.semaprax_main();}}catch(error){{if('{name}'==='failure')continue;throw error;}}{outcome}}}\nconsole.log('byte-slice-for-ok');\n"
                ),
            )
            .unwrap();
            let output = Command::new("node")
                .arg("probe.mjs")
                .current_dir(&web)
                .output()
                .unwrap();
            assert!(
                output.status.success(),
                "{name}: stdout={} stderr={}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout, b"byte-slice-for-ok\n");
        }
    }
    let _ = std::fs::remove_dir_all(&root);
}

#[test]
fn byte_for_view_traversal_materializes_no_buffer_on_core_wasm() {
    let program = parse(VIEW_ONLY, Path::new("byte-slice-traversal-view-only.spx")).unwrap();
    let module = semaprax::wasm::emit_module(&program).unwrap();
    assert_eq!(module, semaprax::wasm::emit_module(&program).unwrap());
    assert!(!module.windows(14).any(|window| window == b"spx_bytes_copy"));
    for payload in wasmparser::Parser::new(0).parse_all(&module) {
        if let wasmparser::Payload::CodeSectionEntry(body) = payload.unwrap() {
            let mut operators = body.get_operators_reader().unwrap();
            while !operators.eof() {
                assert!(!matches!(
                    operators.read().unwrap(),
                    wasmparser::Operator::MemoryCopy { .. }
                        | wasmparser::Operator::MemoryGrow { .. }
                ));
            }
        }
    }
}
