//! Actual generated decoder: hashes only prefilter independently exact equality.
use super::*;

use std::collections::{HashMap, HashSet};

use super::identifier_fixture::app;

/// Decodes one JSON string token with serde_json, never the Semaprax decoder.
fn host_decoded(token: &str) -> String {
    serde_json::from_str(token).expect("host oracle accepts the id token")
}

/// Mirrors the generated identifier fingerprint over host-decoded bytes.
fn host_fingerprint(token: &str) -> usize {
    host_decoded(token)
        .bytes()
        .fold(0, |hash, byte| (hash * 257 + usize::from(byte)) % 65521)
}

fn row(id: &str) -> String {
    format!(r#"{{"id":{id},"arrival":0,"service":1,"priority":0,"deadline":2}}"#)
}

/// One request text and the absolute offset of each record's opening brace.
/// A record's id token opens six bytes later, after `{"id":`.
fn request(rows: &[String]) -> (String, Vec<usize>) {
    let mut text = String::from(r#"{"servers":["S"],"patients":["#);
    let mut offsets = Vec::with_capacity(rows.len());
    for (index, row) in rows.iter().enumerate() {
        if index > 0 {
            text.push(',');
        }
        offsets.push(text.len());
        text.push_str(row);
    }
    text.push_str("]}");
    (text, offsets)
}

/// 256 distinct identifiers in 128 colliding pairs `kNNN-BAA`/`kNNN-ACO`;
/// even pairs escape their leading `k`.
fn colliding_ids() -> Vec<String> {
    (0..128)
        .flat_map(|pair| {
            let lead = if pair % 2 == 0 { "\\u006b" } else { "k" };
            [
                format!("\"{lead}{pair:03}-BAA\""),
                format!("\"k{pair:03}-ACO\""),
            ]
        })
        .collect()
}

/// Max-shape and ordering cases for the bucketed record index. Each case is
/// `(input, expected)`: `Ok((count, last id offset))` or `Err((code, offset,
/// field))`. Host decoding proves every deliberate collision and equality.
type BucketCase = (String, Result<(usize, usize), (i64, usize, i64)>);

fn bucket_cases() -> Vec<BucketCase> {
    let ids = colliding_ids();
    assert_eq!(ids.len(), 256);
    let decoded: HashSet<_> = ids.iter().map(|id| host_decoded(id)).collect();
    assert_eq!(decoded.len(), 256, "colliding ids must stay distinct");
    let mut buckets: HashMap<usize, HashSet<usize>> = HashMap::new();
    for pair in ids.chunks(2) {
        assert_ne!(host_decoded(&pair[0]), host_decoded(&pair[1]));
        assert_eq!(host_fingerprint(&pair[0]), host_fingerprint(&pair[1]));
        let fingerprint = host_fingerprint(&pair[0]);
        buckets
            .entry(fingerprint % 64)
            .or_default()
            .insert(fingerprint);
    }
    // 128 fingerprints in 64 buckets: some chain must reject an unequal one.
    assert!(buckets.values().any(|fingerprints| fingerprints.len() >= 2));
    let first_raw = "\"k000-BAA\"";
    assert_eq!(host_decoded(&ids[0]), host_decoded(first_raw));
    assert_eq!(host_fingerprint("\"ACO\""), host_fingerprint("\"BAA\""));
    assert_eq!(host_decoded("\"\\u0041CO\""), host_decoded("\"ACO\""));

    let rows: Vec<_> = ids.iter().map(|id| row(id)).collect();
    let (exact, offsets) = request(&rows);
    let mut cases = vec![(exact, Ok((256, offsets[255] + 6)))];
    // The 257th item is refused by cardinality before it is decoded or
    // probed, even though it also duplicates the escaped first identifier.
    let mut over = rows.clone();
    over.push(row(first_raw));
    let (over, offsets) = request(&over);
    cases.push((over, Err((8, offsets[256], 2))));
    let mut last = rows;
    last[255] = row(first_raw);
    let (last, offsets) = request(&last);
    cases.push((last, Err((10, offsets[255] + 6, 2))));
    // A colliding, escaped duplicate precedes a later schema fault; an earlier
    // schema fault precedes a later duplicate; grammar precedes both.
    let schema = r#"{"id":"C","arrival":true,"service":1,"priority":0,"deadline":2}"#;
    let (early, offsets) = request(&[
        row("\"ACO\""),
        row("\"BAA\""),
        row("\"\\u0041CO\""),
        schema.to_owned(),
    ]);
    cases.push((early.clone(), Err((10, offsets[2] + 6, 2))));
    let late = format!("{early}?");
    cases.push((late.clone(), Err((1, late.len() - 1, 0))));
    let schema = schema.replace("\"C\"", "\"BAA\"");
    let (first, _) = request(&[row("\"ACO\""), schema, row("\"\\u0041CO\"")]);
    let at = first.find("true").unwrap();
    cases.push((first, Err((5, at, 2))));
    cases
}

fn bucket_app() -> String {
    let mut out = r#"module consumer.app;
use type @id("application.request.json.request-result") from consumer.schema as Outcome;
use type @id("application.patient.json.view") from consumer.schema as PatientView;
use type @id("application.request.json.identifier-span") from consumer.schema as ServerSpan;
use function @id("application.request.json.request.decode") from consumer.schema as decode;
@id("consumer.error") fn error(input:borrow Slice<u8>,expected:i64,at:usize,target:i64)->i64 {
let outcome=decode(input);match own outcome{Outcome::Decoded{servers,patients}=>1,Outcome::Error{code,offset,field}=>if code==expected && offset==at && field==target{0}else{1},}
}
@id("consumer.decoded") fn decoded(input:borrow Slice<u8>,count:usize,last:usize)->i64 {
let outcome=decode(input);match own outcome{Outcome::Error{code,offset,field}=>1,Outcome::Decoded{servers,patients}=>if vec_len<PatientView>(patients)!=count{1}else{let final_row=vec_get<PatientView>(patients,count-1usize);if final_row.id_start==last{0}else{1}},}
}
@id("consumer.main") fn main()->i64 {
let mut failures=0;
"#
    .to_owned();
    for (index, (input, expected)) in bucket_cases().into_iter().enumerate() {
        let call = match expected {
            Ok((count, last)) => {
                format!("decoded(str_as_bytes(view_{index}),{count}usize,{last}usize)")
            }
            Err((code, at, field)) => {
                format!("error(str_as_bytes(view_{index}),{code},{at}usize,{field})")
            }
        };
        out.push_str(&format!(
            "let input_{index}={};let view_{index}=string_as_str(input_{index});failures=failures+{}*{call};\n",
            serde_json::to_string(&input).unwrap(),
            1i64 << index
        ));
    }
    out.push_str("if failures==0{729}else{0-failures}\n}");
    out
}

#[test]
fn generated_identifier_buckets_preserve_max_shape_collisions_and_failure_order() {
    let root = fixture("json-identifier-buckets", SCHEMA);
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
    let admitted = bucket_app();
    // The generated Decoded case has two Vec payload fields. It is outside
    // the private one-record-payload inference profile, so both element type
    // imports must be explicit. Preserve that closed profile as an actual
    // current-source hostile control, then use the ordinary explicit route.
    let span_import = "use type @id(\"application.request.json.identifier-span\") from consumer.schema as ServerSpan;\n";
    assert_eq!(admitted.matches(span_import).count(), 1);
    let omitted = admitted.replacen(span_import, "", 1);
    std::fs::write(root.join("src/app.spx"), canonical(&omitted)).unwrap();
    let refused = project::with_authenticated_project(&root.join("semaprax.toml"), |_| Ok(()))
        .expect_err("omitted element import must retain the private profile refusal");
    assert!(
        refused
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-G172"
                && diagnostic.message
                    == "private layout closure exceeds its independently bounded nominal profile"),
        "{refused:?}"
    );
    std::fs::write(root.join("src/app.spx"), canonical(&admitted)).unwrap();
    project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
        semaprax::hir::validate(snapshot.entry_program()).map_err(|error| vec![error])?;
        // Max-shape inputs use the existing configurable fuel ceiling; the
        // generated decoder's default fuel and physical bounds are unchanged.
        let options = project::ProjectExecutionOptions::new(16 * 1024 * 1024, 160_000_000)
            .map_err(|error| vec![error])?;
        assert_eq!(
            snapshot.execute_entry(&options)?.outcome(),
            &project::ProjectExecutionOutcome::Returned(729)
        );
        let c = codegen::emit_hir_c(snapshot.entry_program()).map_err(|error| vec![error])?;
        for optimization in ["-O0", "-O2"] {
            super::super::super::compile_and_run_c(&c, &root, optimization, "729");
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
        .args(["0", "729", "copy-record-codec"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
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
