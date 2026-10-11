//! The pre-optimization checked kernels are immutable independent oracles.
//! This is ordinary source composition, not a trusted decoder/profile shortcut.
use std::process::Command;

use semaprax::{codegen, project, wasm};

const IMPORTS: &str = r#"module consumer.app;
use function @id("std.data.json.utf8.scalar_at") from std.data.json.utf8 as current_utf8;
use function @id("std.data.json.utf8.sequence_kind") from std.data.json.utf8 as sequence_kind;
use function @id("std.data.json.utf8.offset_at") from std.data.json.utf8 as offset_at;
use function @id("std.data.json.utf8.continuation_at") from std.data.json.utf8 as continuation_at;
use function @id("std.data.json.utf8.is_shortest") from std.data.json.utf8 as is_shortest;
use function @id("std.data.json.query.scalar_at") from std.data.json.query as current_query;
use function @id("std.data.json.escape_kind") from std.data.json as escape_kind;
use function @id("std.data.json.code_unit") from std.data.json as code_unit;
use function @id("std.data.json.at_is") from std.data.json as at_is;
"#;

fn source(body: &str) -> String {
    let mut source = IMPORTS.to_owned();
    for (module, original) in [
        ("utf8", include_str!("json_scalar_lazy/utf8_original.txt")),
        ("query", include_str!("json_scalar_lazy/query_original.txt")),
    ] {
        source.push_str(
            &original
                .replace(
                    &format!("std.data.json.{module}.scalar_at"),
                    &format!("consumer.original.{module}"),
                )
                .replace("fn scalar_at(", &format!("fn original_{module}(")),
        );
    }
    source.push_str(r#"@id("consumer.check") fn check(input: borrow Slice<u8>, start: usize) -> bool {
        current_utf8(input,start)==original_utf8(input,start) && current_query(input,start)==original_query(input,start)
    }
    @id("consumer.main") fn main() -> i64 {
"#);
    source.push_str(body);
    source.push_str("}\n");
    source
}

fn install(name: &str, source: &str) -> std::path::PathBuf {
    let root = super::super::temporary(name);
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(root.join("semaprax.toml"), "schema = \"semaprax.manifest.v1\"\n\n[package]\nname = \"scalar-differential\"\nversion = \"0.1.0\"\nprofile = \"useful-data.v1\"\n\n[modules]\nentry = \"consumer.app\"\nsources = [\"src/app.spx\", \"src/tests.spx\"]\ntests = [\"consumer.tests\"]\n\n[exports]\nweb = [\"consumer.main\"]\n\n[dependencies]\nstd.data.json = \"=0.1.0\"\nstd.data.json.query = \"=0.1.0\"\nstd.data.json.utf8 = \"=0.1.0\"\n").unwrap();
    std::fs::write(
        root.join("src/tests.spx"),
        semaprax::format::canonical(
            &semaprax::parse(
                "module consumer.tests; @id(\"consumer.tests.main\") fn main() -> i64 { 0 }\n",
                "scalar-tests.spx",
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let parsed = semaprax::parse(source, "scalar-differential.spx").unwrap();
    let canonical = semaprax::format::canonical(&parsed);
    assert_eq!(
        canonical,
        semaprax::format::canonical(&semaprax::parse(&canonical, "roundtrip.spx").unwrap())
    );
    std::fs::write(root.join("src/app.spx"), canonical).unwrap();
    root
}

fn array(bytes: &[u8]) -> String {
    format!(
        "[{}]",
        bytes
            .iter()
            .map(|byte| format!("{byte}u8"))
            .collect::<Vec<_>>()
            .join(",")
    )
}

#[test]
fn lazy_scalar_kernels_match_original_on_all_bytes_unicode_and_absent_offsets() {
    let mut body = format!(
        "let all={};let view=array_as_slice(all);let mut at=0usize;let mut bad=0;while at<=256usize {{bad=bad+if check(view,at){{0}}else{{1}};at=at+1usize;0}}\n",
        array(&(0..=255).collect::<Vec<u8>>())
    );
    let cases: &[&[u8]] = &[
        b"\\\"",
        b"\\\\",
        b"\\/",
        b"\\b",
        b"\\f",
        b"\\n",
        b"\\r",
        b"\\t",
        b"\\q",
        b"\\",
        b"\\u0000",
        b"\\u007f",
        b"\\u0080",
        b"\\u07ff",
        b"\\u0800",
        b"\\ud7ff",
        b"\\ue000",
        b"\\uffff",
        b"\\ud800\\udc00",
        b"\\udbff\\udfff",
        b"\\ud800",
        b"\\udc00",
        b"\\ud800\\u0000",
        b"\\ud800\\q0000",
        b"\\u0g00",
        b"\\u000",
        b"\\u",
        b"plain",
        &[0xc2, 0x80],
        &[0xdf, 0xbf],
        &[0xe0, 0xa0, 0x80],
        &[0xed, 0x9f, 0xbf],
        &[0xef, 0xbf, 0xbf],
        &[0xf0, 0x90, 0x80, 0x80],
        &[0xf4, 0x8f, 0xbf, 0xbf],
        &[0xc0, 0x80],
        &[0xe0, 0x80, 0x80],
        &[0xed, 0xa0, 0x80],
        &[0xf0, 0x80, 0x80, 0x80],
        &[0xf4, 0x90, 0x80, 0x80],
        &[0xff],
        &[0xe2, 0x82],
    ];
    // Separate lexical blocks retain the ordinary binding/extent limits.
    for (i, bytes) in cases.iter().enumerate() {
        body.push_str(&format!("let failed_{i}={{let raw={};let input=array_as_slice(raw);let mut index=0usize;let mut count=0;while index<={}usize{{count=count+if check(input,index){{0}}else{{1}};index=index+1usize;0}}count}};bad=bad+failed_{i};\n", array(bytes),bytes.len()));
    }
    body.push_str("let empty=byte_range(view,0usize,0usize);bad=bad+if check(empty,0usize)&&check(empty,18446744073709551604usize)&&check(empty,18446744073709551609usize)&&current_query(empty,18446744073709551609usize)==-1{0}else{1};if bad==0{0}else{1}\n");
    let root = install("json-scalar-lazy", &source(&body));
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        snapshot.check()?;
        assert_eq!(
            snapshot
                .execute_entry(&project::ProjectExecutionOptions::default())?
                .outcome(),
            &project::ProjectExecutionOutcome::Returned(0)
        );
        for id in [
            "std.data.json.utf8.scalar_at",
            "std.data.json.query.scalar_at",
            "consumer.original.utf8",
            "consumer.original.query",
        ] {
            assert!(snapshot.semantic_graph().contains(id));
        }
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        for optimization in ["-O0", "-O2"] {
            super::super::compile_and_run_c(&c, &root, optimization, "0");
        }
        let bytes =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        std::fs::write(root.join("scalar.wasm"), bytes).unwrap();
        std::fs::write(
            root.join("scalar.mjs"),
            super::super::wasm_host::wasm_conformance_js("scalar.wasm", 0, 0, Some(0)),
        )
        .unwrap();
        let output = Command::new("node")
            .arg("scalar.mjs")
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        Ok(())
    })
    .unwrap();
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn near_max_escape_offsets_preserve_original_normalized_arithmetic_failure() {
    let mut expected = None;
    for function in ["original_query", "current_query"] {
        let root = install(
            function,
            &source(&format!(
                "let raw=[0u8];{function}(array_as_slice(raw),18446744073709551615usize)"
            )),
        );
        project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            let result = snapshot.execute_entry(&project::ProjectExecutionOptions::default())?;
            let project::ProjectExecutionOutcome::LanguageFailure(status) = result.outcome() else {
                panic!("missing arithmetic failure: {result:?}")
            };
            assert_eq!(
                status.class(),
                semaprax::conformance::StatusClass::Arithmetic
            );
            assert_eq!(
                status.domain_id(),
                semaprax::conformance::ARITHMETIC_STATUS_DOMAIN_V1
            );
            let actual = (status.code(), status.retryability());
            if let Some(expected) = &expected {
                assert_eq!(&actual, expected);
            } else {
                expected = Some(actual);
            }
            Ok(())
        })
        .unwrap();
        std::fs::remove_dir_all(root).unwrap();
    }
}
