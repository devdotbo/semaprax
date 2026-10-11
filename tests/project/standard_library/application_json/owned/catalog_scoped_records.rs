//! Additive v31 Catalog source: unchanged 23-case functional oracle, not v30 qualification.
use super::public_example::{compile_native, execute};
use super::*;
use std::collections::BTreeSet;
use std::path::Path;

const EXAMPLE: &str = "examples/catalog-scoped-record-successor";
const SPEC: &[u8] = include_bytes!("../../../../../benchmarks/catalog-tokens-v1/SPEC.md");
const CORPUS: &str =
    include_str!("../../../../../benchmarks/catalog-tokens-v1/acceptance/corpus.json");
const SOURCES: &[(&str, &str)] = &[
    (
        "src/app.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/app.spx"),
    ),
    (
        "src/app.command.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/app.command.spx"),
    ),
    (
        "src/request.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/request.spx"),
    ),
    (
        "src/response.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/response.spx"),
    ),
    (
        "src/model.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/model.spx"),
    ),
    (
        "src/order.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/order.spx"),
    ),
    (
        "src/restock.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/restock.spx"),
    ),
    (
        "src/publication.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/publication.spx"),
    ),
    (
        "src/tests.spx",
        include_str!("../../../../../examples/catalog-scoped-record-successor/src/tests.spx"),
    ),
];

fn hex(value: &str) -> Vec<u8> {
    assert_eq!(value.len() % 2, 0);
    value
        .as_bytes()
        .chunks_exact(2)
        .map(|pair| {
            let digits = std::str::from_utf8(pair).unwrap();
            u8::from_str_radix(digits, 16).unwrap()
        })
        .collect()
}

fn stage(root: &Path) {
    std::fs::create_dir_all(root.join("src")).unwrap();
    let original = Path::new(env!("CARGO_MANIFEST_DIR")).join(EXAMPLE);
    std::fs::copy(original.join("semaprax.toml"), root.join("semaprax.toml")).unwrap();
    for (path, source) in SOURCES {
        let canonical_source = canonical(source);
        assert_eq!(canonical(&canonical_source), canonical_source, "{path}");
        std::fs::write(root.join(path), canonical_source).unwrap();
    }
    // Derive/replay both actual generators from the unchanged bootstrap.
    // No guessed body, source-origin exemption, or half-installed import is used.
    let mut candidates = Vec::new();
    for (path, identity, profile) in [
        (
            "src/request.spx",
            "catalog.request",
            project::JsonCodecProfile::StreamUtf8OwnedRequest {
                max_string_bytes: 16,
            },
        ),
        (
            "src/response.spx",
            "catalog.report",
            project::JsonCodecProfile::CollectionResponse {
                max_string_bytes: 16,
            },
        ),
    ] {
        let generated =
            project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
                project::derive_json_codec_source_with_profile(
                    &snapshot.retain_revision(),
                    path,
                    identity,
                    profile,
                )
            })
            .unwrap();
        assert_eq!(canonical(&generated), generated);
        project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            project::verify_json_codec_source_with_profile(
                &snapshot.retain_revision(),
                path,
                identity,
                &generated,
                profile,
            )
        })
        .unwrap();
        candidates.push((path, generated));
    }
    for (path, generated) in candidates {
        std::fs::write(root.join(path), generated).unwrap();
    }
    // The original ordinary Input→Decode tail now composes. Preserve its
    // independent projected-owner refusal against the same generated schemas.
    let original = include_str!("catalog-scoped-record-command-original.txt");
    std::fs::write(root.join("src/app.spx"), canonical(original)).unwrap();
    let errors =
        project::with_authenticated_project(&root.join("semaprax.toml"), |_| Ok(())).unwrap_err();
    {
        let code = "SPX-O118";
        assert!(
            errors.iter().any(|error| error.code == code),
            "{code}: {errors:?}"
        );
    }
    assert!(
        !errors
            .iter()
            .any(|error| matches!(error.code, "SPX-T258" | "SPX-T216")),
        "{errors:?}"
    );
    std::fs::copy(root.join("src/app.command.spx"), root.join("src/app.spx")).unwrap();
}

#[test]
fn typed_catalog_derives_codecs_and_preserves_all_23_functional_cases() {
    let corpus: serde_json::Value = serde_json::from_str(CORPUS).unwrap();
    assert_eq!(corpus["schema"], "catalog.frozen-manual-corpus.v1");
    let digest = format!("{:x}", semaprax::digest_hex::LowerHex(Sha256::digest(SPEC)));
    assert_eq!(corpus["spec_sha256"], digest);
    let cases = corpus["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 23);
    assert_eq!(
        cases
            .iter()
            .map(|case| case["name"].as_str().unwrap())
            .collect::<BTreeSet<_>>()
            .len(),
        23
    );
    let root = crate::standard_library::temporary("catalog-scoped-record-successor");
    stage(&root);
    let native = root.join("catalog-native");
    let c_source = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        // Explicit successor route; this assertion must never be relabeled v30.
        assert_eq!(snapshot.manifest().schema(), project::PROJECT_SCHEMA_V31);
        assert_eq!(
            snapshot.manifest().project_profile(),
            project::ProjectProfile::StdinStreamCollectionRecordCommandIoV1
        );
        let graph = snapshot.semantic_graph();
        assert!(graph.contains("core.vec.str-at"));
        assert!(!graph.contains("\"catalog.tag\""));
        super::super::collection_response::assert_scoped_row_reads(
            snapshot.public_api_program(),
            "catalog.output_item",
        );
        assert!(graph.contains("catalog.request.json.utf8.owned.decode"));
        assert!(graph.contains("catalog.report.json.collection-response.encode"));
        let tests = snapshot.execute_test(&project::ProjectExecutionOptions::default())?;
        assert!(tests.command_succeeded());
        assert_eq!(tests.cases().len(), 2);
        snapshot.build_native(&native)?;
        codegen::emit_hir_c_with_stdin_stream_collection_records(
            snapshot.public_api_program(),
            "catalog.command",
        )
        .map_err(|error| vec![error])
    })
    .unwrap();
    // Differential source retains the original ordinary normalized-input join,
    // while retaining the admitted owned-record command destructuring. The old
    // receive/decide workaround remains independently executed below.
    let workaround = std::fs::read_to_string(root.join("src/app.spx")).unwrap();
    let original_receive = "fn receive(bytes: own Bytes, length: usize) -> Decode\n{\n    decode(byte_range(bytes_as_slice(bytes), 0usize, length))\n}";
    assert!(workaround.contains(original_receive));
    let old_command = "let normalized = normalize();\n    match own normalized { StreamInput::Error { code: _code, offset: _offset, field: _field } => reject(), StreamInput::Ready { bytes, length } => { let outcome = receive(bytes, length); decide(outcome) }, }";
    assert!(workaround.contains(old_command));
    let joined = workaround.replace(original_receive,
        "fn receive() -> Decode uses { process.stdin.read } {let normalized=normalize();match own normalized {StreamInput::Error{code,offset,field}=>Decode::Error{code:code,offset:offset,field:field},StreamInput::Ready{bytes,length}=>decode(byte_range(bytes_as_slice(bytes),0usize,length)),}}")
        .replace(old_command,"decide(receive())");
    std::fs::write(root.join("src/app.spx"), canonical(&joined)).unwrap();
    let joined_c = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        codegen::emit_hir_c_with_stdin_stream_collection_records(
            snapshot.public_api_program(),
            "catalog.command",
        )
        .map_err(|error| vec![error])
    })
    .unwrap();
    std::fs::write(root.join("src/app.spx"), workaround).unwrap();
    super::string_slot_adoption::catalog(&root);
    let wrapper_c = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        assert!(snapshot.semantic_graph().contains("catalog.tag"));
        codegen::emit_hir_c_with_stdin_stream_collection_records(
            snapshot.public_api_program(),
            "catalog.command",
        )
        .map_err(|error| vec![error])
    })
    .unwrap();
    // All three sources execute every frozen raw-byte/all-channel row unchanged.
    for (route, source) in [
        ("slot-views", &c_source),
        ("joined", &joined_c),
        ("tag-wrapper", &wrapper_c),
    ] {
        for optimization in ["-O0", "-O2"] {
            let binary = compile_native(source, &root, optimization);
            for case in cases {
                let name = case["name"].as_str().unwrap();
                let output = execute(&binary, &hex(case["input_hex"].as_str().unwrap()));
                assert_eq!(
                    output.status.code(),
                    Some(i32::try_from(case["status"].as_i64().unwrap()).unwrap()),
                    "{route}: {optimization}: {name}"
                );
                assert_eq!(
                    output.stdout,
                    hex(case["stdout_hex"].as_str().unwrap()),
                    "{route}: {optimization}: {name}"
                );
                assert_eq!(
                    output.stderr,
                    hex(case["stderr_hex"].as_str().unwrap()),
                    "{route}: {optimization}: {name}"
                );
            }
        }
    }
    // Also bind the ordinary Project native-build artifact to every original row.
    for case in cases {
        let output = execute(&native, &hex(case["input_hex"].as_str().unwrap()));
        assert_eq!(
            output.status.code(),
            Some(i32::try_from(case["status"].as_i64().unwrap()).unwrap())
        );
        assert_eq!(output.stdout, hex(case["stdout_hex"].as_str().unwrap()));
        assert_eq!(output.stderr, hex(case["stderr_hex"].as_str().unwrap()));
    }
    std::fs::remove_dir_all(root).unwrap();
}

#[path = "catalog_sort_adoption.rs"]
mod catalog_sort_adoption;

#[test]
fn typed_vec_constructor_retains_exact_explicit_chain_application_control() {
    let root = crate::standard_library::temporary("catalog-vec-constructor-control");
    stage(&root);
    let original = include_str!("vec_constructor_adoption/catalog_tests_explicit_chain.txt");
    let adopted = SOURCES
        .iter()
        .find(|(path, _)| *path == "src/tests.spx")
        .unwrap()
        .1;
    assert!(!original.contains("vec_of<"));
    assert!(adopted.contains("vec_of<InputItem>"));
    super::vec_constructor_adoption::assert_application_control(&root, adopted, original);
    std::fs::remove_dir_all(root).unwrap();
}
