//! Focused collision/64/65/256/ordering cases on the actual checked app.
//! Only the test entry is replaced; no published corpus is repeated here.
use super::super::{application_options, codegen, format, project};
use super::*;
use std::path::Path;
use std::process::Command;

#[test]
fn decoded_index_collision_boundaries_and_failure_order_run_on_native_and_wasm() {
    let cases = index_cases();
    assert_eq!(cases.len(), 21);
    let scratch = ScratchRoot::new();
    // Bounded test-entry graphs, sharing all unchanged production modules.
    for (group, cases) in cases.chunks(5).enumerate() {
        let root = scratch.path().join(format!("decoded-index-{group}"));
        copy_fixture(&fixture(), &root);
        let mut source = String::from("module catalog_normalizer.tests;\nuse function @id(\"catalog_normalizer.app.normalizes-to\") from catalog_normalizer.app as normalizes_to;\nuse function @id(\"catalog_normalizer.app.normalizes-enriched-to\") from catalog_normalizer.app as normalizes_enriched_to;\n@id(\"catalog_normalizer.tests.smoke\") fn smoke()->bool {true}\n@id(\"catalog_normalizer.tests.main\") fn main()->i64 {let mut failed=0;\n");
        for (index, case) in cases.iter().enumerate() {
            let expected = run_oracle_with(&case.input, case.enriched);
            let response: serde_json::Value = serde_json::from_slice(&expected).unwrap();
            if let Some(category) = case.category {
                assert_eq!(response["status"], "error", "{}", case.name);
                assert_eq!(response["category"], category, "{}", case.name);
                assert_eq!(
                    response["record_index"].as_u64(),
                    case.record_index,
                    "{}",
                    case.name
                );
            } else {
                assert_eq!(response["status"], "ok", "{}", case.name);
            }
            let input = std::str::from_utf8(&case.input).unwrap();
            let expected = std::str::from_utf8(&expected).unwrap();
            let call = if case.enriched {
                "normalizes_enriched_to"
            } else {
                "normalizes_to"
            };
            source.push_str(&format!("let input_{index}={};let expected_{index}={};let input_view_{index}=string_as_str(input_{index});let expected_view_{index}=string_as_str(expected_{index});failed=failed+if {call}(str_as_bytes(input_view_{index}),str_as_bytes(expected_view_{index})) {{0}}else{{{}}};\n", serde_json::to_string(input).unwrap(), serde_json::to_string(expected).unwrap(), 1i64 << index));
        }
        source.push_str("failed}\n");
        let path = root.join("src/tests.spx");
        let (ast, comments) = semaprax::parse_with_comments(&source, &path).unwrap();
        std::fs::write(
            &path,
            format::comments::canonical_with_comments(&ast, &comments),
        )
        .unwrap();
        project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            snapshot.check()?;
            assert_eq!(
                snapshot.execute_test(&application_options())?.outcome(),
                &project::ProjectExecutionOutcome::Returned(0),
                "decoded-index group {group}: {}",
                cases.iter().map(|case| case.name).collect::<Vec<_>>().join(", ")
            );
            let c = codegen::emit_hir_c(snapshot.test_program()).map_err(|error| vec![error])?;
            for optimization in ["-O0", "-O2"] {
                let path = scratch
                    .path()
                    .join(format!("index-{group}-{optimization}.c"));
                let executable = scratch.path().join(format!("index-{group}-{optimization}"));
                std::fs::write(&path, &c).unwrap();
                let build = Command::new("clang")
                    .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
                    .arg(&path)
                    .arg("-o")
                    .arg(&executable)
                    .output()
                    .unwrap();
                assert!(
                    build.status.success(),
                    "{}",
                    String::from_utf8_lossy(&build.stderr)
                );
                let run = Command::new(&executable).output().unwrap();
                assert!(
                    run.status.success(),
                    "{}",
                    String::from_utf8_lossy(&run.stderr)
                );
                assert_eq!(run.stdout, b"0\n");
            }
            let wasm = scratch.path().join(format!("index-{group}.wasm"));
            std::fs::write(&wasm, snapshot.test_wasm_module()?).unwrap();
            let script = scratch.path().join(format!("index-{group}.mjs"));
            let test_root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/useful_data");
            let host = format!(
                "{}\n{}",
                std::fs::read_to_string(test_root.join("environment_provider_fixture.mjs"))
                    .unwrap(),
                std::fs::read_to_string(test_root.join("catalog_normalizer_application.mjs"))
                    .unwrap()
            );
            std::fs::write(&script, host).unwrap();
            let run = Command::new("node")
                .arg(&script)
                .arg(&wasm)
                .output()
                .unwrap();
            assert!(
                run.status.success(),
                "{}",
                String::from_utf8_lossy(&run.stderr)
            );
            Ok(())
        })
        .unwrap();
    }
}
