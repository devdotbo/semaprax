//! Actual generated decoder: hashes only prefilter independently exact equality.
use super::*;

fn offset(input: &[u8], token: &[u8]) -> usize {
    input
        .windows(token.len())
        .rposition(|window| window == token)
        .unwrap()
}
fn app() -> String {
    let mut out = r#"module consumer.app;
use type @id("application.request.json.request-result") from consumer.schema as Outcome;
use type @id("application.patient.json.view") from consumer.schema as PatientView;
use type @id("application.request.json.identifier-span") from consumer.schema as ServerSpan;
use function @id("application.request.json.request.decode") from consumer.schema as decode;
use function @id("application.patient.json.identifier-fingerprint") from consumer.schema as fingerprint;
@id("consumer.error") fn error(input:borrow Slice<u8>,expected:i64,at:usize,target:i64)->i64 {
let outcome=decode(input);match own outcome{Outcome::Decoded{servers,patients}=>1,Outcome::Error{code,offset,field}=>if code==expected && offset==at && field==target{0}else{1},}
}
@id("consumer.main") fn main()->i64 {
let first=[34u8,66u8,65u8,65u8,34u8];let second=[34u8,65u8,67u8,79u8,34u8];
let collision=fingerprint(array_as_slice(first),0usize,5usize)==fingerprint(array_as_slice(second),0usize,5usize);
let mut failures=if collision{0}else{1};
"#.to_owned();
    let valid = br#"{"servers":["BAA","ACO"],"patients":[{"id":"BAA","arrival":0,"service":1,"priority":0,"deadline":2},{"id":"ACO","arrival":0,"service":1,"priority":0,"deadline":2}]}"#;
    out.push_str(&format!("let valid={};let outcome=decode(array_as_slice(valid));failures=failures+match own outcome{{Outcome::Error{{code,offset,field}}=>1,Outcome::Decoded{{servers,patients}}=>if vec_len<ServerSpan>(servers)==2usize && vec_len<PatientView>(patients)==2usize{{0}}else{{1}},}};\n", array(valid)));
    let rows = br#"{"servers":["S"],"patients":[{"id":"BAA","arrival":0,"service":1,"priority":0,"deadline":2},{"id":"ACO","arrival":0,"service":1,"priority":0,"deadline":2},{"id":"\u0042AA","arrival":0,"service":1,"priority":0,"deadline":2}]}"#;
    let cases: Vec<(&[u8], i64, usize, i64)> = vec![
        (
            br#"{"servers":["BAA","\u0042AA"],"patients":[]}"#,
            10,
            offset(
                br#"{"servers":["BAA","\u0042AA"],"patients":[]}"#,
                br#""\u0042AA""#,
            ),
            1,
        ),
        (rows, 10, offset(rows, br#""\u0042AA""#), 2),
        (br#"{"servers":["\u0000"],"patients":[]}"#, 6, 12, 1),
        (br#"{"servers":["\ud83d\ude00"],"patients":[]}"#, 6, 12, 1),
        ("{\"servers\":[\"é\"],\"patients\":[]}".as_bytes(), 6, 12, 1),
        (
            "{\"servers\":[\"😀\"],\"patients\":[]}".as_bytes(),
            6,
            12,
            1,
        ),
    ];
    for (index, (input, code, at, field)) in cases.into_iter().enumerate() {
        out.push_str(&format!("let bad_{index}={};failures=failures+error(array_as_slice(bad_{index}),{code},{at}usize,{field});\n",array(input)));
    }
    // Full grammar validation still outranks the earlier duplicate/schema fault.
    let malformed = br#"{"servers":["BAA","\u0042AA"],"patients":[]}?"#;
    out.push_str(&format!("let late={};failures=failures+error(array_as_slice(late),1,{}usize,0);if failures==0{{728}}else{{0-failures}}\n}}",array(malformed),malformed.len()-1));
    out
}

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
