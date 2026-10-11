//! Actual generated decoder: hashes only prefilter independently exact equality.
use super::*;

use super::identifier_fixture::app;

#[test]
fn generated_identifier_prefilter_preserves_collisions_equivalent_spelling_and_domain_refusals() {
    let root = fixture("json-identifier-prefilter", SCHEMA);
    let source = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        let revision = snapshot.retain_revision();
        let profile = project::JsonCodecProfile::RequestViews;
        let source = project::derive_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "application.request",
            profile,
        )?;
        project::verify_json_codec_source_with_profile(
            &revision,
            "src/schema.spx",
            "application.request",
            &source,
            profile,
        )?;
        assert_eq!(canonical(&source), source);
        Ok(source)
    })
    .unwrap();
    std::fs::write(root.join("src/schema.spx"), source).unwrap();
    std::fs::write(root.join("src/app.spx"), canonical(&app())).unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert!(snapshot
            .retain_revision()
            .semantic_graph()
            .contains("application.patient.json.identifier-fingerprint"));
        semaprax::hir::validate(snapshot.entry_program()).map_err(|error| vec![error])?;
        assert_eq!(
            snapshot
                .execute_entry(&project::ProjectExecutionOptions::default())?
                .outcome(),
            &project::ProjectExecutionOutcome::Returned(728)
        );
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        for optimization in ["-O0", "-O2"] {
            super::super::super::compile_and_run_c(&c, &root, optimization, "728");
        }
        let bytes =
            wasm::emit_resolved_module(snapshot.entry_program()).map_err(|error| vec![error])?;
        wasmparser::Validator::new().validate_all(&bytes).unwrap();
        std::fs::write(root.join("app.wasm"), bytes).unwrap();
        Ok(())
    })
    .unwrap();
    let host = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/owned_data/owned_leaf_vec/host.js");
    let output = Command::new("node")
        .arg(host)
        .arg(root.join("app.wasm"))
        .args(["0", "728", "copy-record-codec"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}
