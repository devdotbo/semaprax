use super::saved_page::page;
use super::*;
fn revision(field: &str) -> ProjectRevision {
    revision_with_padding(field, None)
}
fn revision_with_padding(field: &str, padded_path: Option<&str>) -> ProjectRevision {
    let manifest = crate::project::ProjectManifest::parse(include_str!(
        "../../../../examples/calculator-project/semaprax.toml"
    ))
    .unwrap();
    // Canonical projection writes type declarations before functions, so the
    // record sits directly below the module line of the unchanged example.
    let functions = include_str!("../../../../examples/calculator-project/src/core.spx")
        .strip_prefix("module calculator.core;\n")
        .unwrap();
    let core = format!(
        "module calculator.core;\n\n@id(\"explain.Row\")\nrecord Row {{\n    @id(\"explain.Row.value\")\n    value: {field},\n}}\n{functions}"
    );
    let sources = [
        (
            "src/app.spx",
            include_str!("../../../../examples/calculator-project/src/app.spx").to_owned(),
        ),
        ("src/core.spx", core),
        (
            "src/tests.spx",
            include_str!("../../../../examples/calculator-project/src/tests.spx").to_owned(),
        ),
    ]
    .into_iter()
    .map(|(path, mut source)| {
        if padded_path == Some(path) {
            // A canonical end-of-file comment follows the final `}` directly.
            source.push_str("//");
            source.push_str(&"x".repeat(MAX_SCHEMA_BYTES + 1));
            source.push('\n');
        }
        SemanticWorkspaceSource {
            path: path.into(),
            source,
        }
    })
    .collect();
    let built = crate::project::build::build_owned(&manifest, sources).unwrap();
    ProjectRevision::from_built(manifest, built)
}
#[test]
fn actual_checked_type_reports_all_profiles_and_keeps_shape_separate_from_candidate_admission() {
    let revision = revision("i64");
    let options = JsonCodecSuitabilityOptions {
        record: "explain.Row".into(),
        max_bytes: 16384,
        ..Default::default()
    };
    let output = explain_json_codec_suitability(&revision, &options).unwrap();
    let value: Value = serde_json::from_str(&output).unwrap();
    assert_eq!(value["project_revision"], revision.project_revision());
    assert_eq!(value["source"], "src/core.spx");
    assert_eq!(value["type"], "explain.Row");
    assert_eq!(value["profile_count"], 12);
    assert!(value["policy"]["max_string_bytes"].is_null());
    assert!(value["policy"]["max_array_items"].is_null());
    assert_eq!(value["coverage"], "complete");
    assert!(value["next_offset"].is_null());
    let rows = value["profiles"].as_array().unwrap();
    assert_eq!(rows.len(), 12);
    assert_eq!(rows[0]["selector"], "flat-scalars.v1");
    assert_eq!(rows[0]["shape_matches"], true);
    assert!(!rows[0]["required_modules"].as_array().unwrap().is_empty());
    assert!(!rows[0]["imports_absent_from_current_entry_index"]
        .as_array()
        .unwrap()
        .is_empty());
    for row in rows {
        assert_eq!(row["candidate_admission"], "not_evaluated");
        assert_eq!(row["backend_admission"], "not_evaluated");
    }
    assert!(rows.iter().any(
        |r| r["shape_matches"].is_null() && !r["missing_policy"].as_array().unwrap().is_empty()
    ));
    // The unchanged generator refuses to install missing imports even when its
    // scalar shape matches; inspection is never a candidate-admission receipt.
    assert!(derive_json_codec_source(&revision, "src/core.spx", "explain.Row").is_err());
    assert_eq!(
        output,
        explain_json_codec_suitability(&revision, &options).unwrap()
    );
    let mut paged = options.clone();
    paged.max_bytes = 2048;
    paged.revision = Some(revision.project_revision().into());
    let mut visited = Vec::new();
    loop {
        let output = explain_json_codec_suitability(&revision, &paged).unwrap();
        assert!(output.len() <= paged.max_bytes);
        let page: Value = serde_json::from_str(&output).unwrap();
        assert_eq!(page["source_sha256"], value["source_sha256"]);
        visited.extend(page["profiles"].as_array().unwrap().iter().cloned());
        let Some(next) = page["next_offset"].as_u64() else {
            break;
        };
        assert!(next as usize > paged.offset);
        paged.offset = next as usize;
    }
    assert_eq!(&visited, rows);
}
#[test]
fn revision_path_identity_bounds_and_target_policy_are_independent_refusals() {
    let original = revision("i64");
    let base = JsonCodecSuitabilityOptions {
        record: "explain.Row".into(),
        max_bytes: 16384,
        ..Default::default()
    };
    // An unrelated authored module is not the selected schema's byte budget.
    let unrelated = revision_with_padding("i64", Some("src/app.spx"));
    assert!(unrelated
        .sources()
        .iter()
        .any(|source| source.path() == "src/app.spx" && source.source().len() > MAX_SCHEMA_BYTES));
    let implicit = explain_json_codec_suitability(&unrelated, &base).unwrap();
    let mut explicit = base.clone();
    explicit.source = Some("src/core.spx".into());
    assert_eq!(
        implicit,
        explain_json_codec_suitability(&unrelated, &explicit).unwrap()
    );
    // The exact defining source still obeys the unchanged schema ceiling.
    let oversized_schema = revision_with_padding("i64", Some("src/core.spx"));
    for options in [&base, &explicit] {
        assert!(explain_json_codec_suitability(&oversized_schema, options).is_err());
    }
    let mut stale = base.clone();
    stale.revision = Some(original.project_revision().into());
    assert!(explain_json_codec_suitability(&revision("usize"), &stale).is_err());
    for change in 0..6 {
        let mut o = base.clone();
        match change {
            0 => o.record = "absent".into(),
            1 => o.source = Some("src/absent.spx".into()),
            2 => o.offset = 1,
            3 => o.max_string_bytes = Some(65),
            4 => o.max_array_items = Some(257),
            _ => o.target = Some("hosted".into()),
        }
        assert!(
            explain_json_codec_suitability(&original, &o).is_err(),
            "{change}"
        );
    }
    for target in ["native64", "wasm32"] {
        let mut o = base.clone();
        o.target = Some(target.into());
        let value: Value =
            serde_json::from_str(&explain_json_codec_suitability(&original, &o).unwrap()).unwrap();
        assert_eq!(value["target"], target);
        assert_eq!(value["target_manifest_policy"]["manifest_allows"], true);
        assert!(value["profiles"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["backend_admission"] == "not_evaluated"));
    }
}
#[test]
fn exact_byte_budget_and_one_short_continue_without_dropping_profile_rows() {
    let rows = (0..12)
        .map(|i| json!({"selector":format!("policy-{i}"),"shape_matches":i%2==0}))
        .collect::<Vec<_>>();
    let envelope = json!({"schema":"test-boundary","profiles":[],"offset":0,"next_offset":Value::Null,"coverage":"continued"});
    let full = page(envelope.clone(), &rows, 0, 16384).unwrap();
    assert_eq!(page(envelope.clone(), &rows, 0, full.len()).unwrap(), full);
    assert_eq!(
        page(envelope.clone(), &rows, 0, full.len() + 1).unwrap(),
        full
    );
    let short = page(envelope.clone(), &rows, 0, full.len() - 1).unwrap();
    assert!(short.len() < full.len());
    let short: Value = serde_json::from_str(&short).unwrap();
    let next = short["next_offset"].as_u64().unwrap() as usize;
    assert_eq!(short["profiles"].as_array().unwrap(), &rows[..next]);
    assert_eq!(short["coverage"], "continued");
    let remainder: Value =
        serde_json::from_str(&page(envelope.clone(), &rows, next, 16384).unwrap()).unwrap();
    assert_eq!(remainder["profiles"].as_array().unwrap(), &rows[next..]);
    assert!(remainder["next_offset"].is_null());
    assert!(page(envelope.clone(), &rows, 12, 16384).is_err());
    assert!(page(envelope, &rows, 0, 1).is_err());
}

#[test]
fn compact_inventory_paging_matches_saved_eager_bytes_and_renders_only_page_rows() {
    let rows = (0..12)
        .map(|i| json!({"selector":format!("policy-{i}"),"shape_matches":i%2==0,"reason":"é\\\""}))
        .collect::<Vec<_>>();
    let envelope = json!({"schema":"test-boundary","profiles":[],"offset":0,"next_offset":Value::Null,"coverage":"continued"});
    // Observe only paging and its output Drop, excluding the independent
    // input inventory. This proves a local allocation reduction, not the
    // fragment generators' or complete Project's memory requirements.
    let (_, eager) = crate::test_allocation_observer::measure(|| {
        drop(page(envelope.clone(), &rows, 0, 16384).unwrap());
    });
    let (_, compact) = crate::test_allocation_observer::measure(|| {
        drop(
            paging::page(envelope.clone(), rows.len(), 0, 16384, |index| {
                rows[index].clone()
            })
            .unwrap(),
        );
    });
    assert!(!eager.invalid && !compact.invalid);
    assert_eq!((eager.live, compact.live), (0, 0));
    assert!(compact.allocated < eager.allocated);
    for offset in 0..12 {
        let mut envelope = envelope.clone();
        envelope["offset"] = json!(offset);
        let full = page(envelope.clone(), &rows, offset, 16384).unwrap();
        for maximum in [1, full.len() - 1, full.len(), full.len() + 1, 16384] {
            let mut rendered = Vec::new();
            let actual = paging::page(envelope.clone(), rows.len(), offset, maximum, |index| {
                rendered.push(index);
                rows[index].clone()
            });
            let expected = page(envelope.clone(), &rows, offset, maximum);
            match (actual, expected) {
                (Ok(actual), Ok(expected)) => {
                    assert_eq!(actual, expected);
                    let output: Value = serde_json::from_str(&actual).unwrap();
                    let accepted = output["profiles"].as_array().unwrap().len();
                    assert_eq!(
                        rendered,
                        (offset..offset + accepted + usize::from(offset + accepted < rows.len()))
                            .collect::<Vec<_>>()
                    );
                }
                (Err(actual), Err(expected)) => assert_eq!(actual, expected),
                _ => panic!("paging verdict differs"),
            }
        }
    }
}

#[test]
fn compact_profile_facts_preserve_all_twelve_saved_predicates_imports_and_refusals() {
    for field in ["i64", "string", "bool"] {
        let revision = revision(field);
        let source = revision
            .sources()
            .iter()
            .find(|s| s.path() == "src/core.spx")
            .unwrap();
        let program = crate::parse(source.source(), source.path()).unwrap();
        let declaration = program
            .types
            .iter()
            .find(|t| t.stable_id == "explain.Row")
            .unwrap();
        for policy in [None, Some(1), Some(64)] {
            let options = JsonCodecSuitabilityOptions {
                record: "explain.Row".into(),
                max_string_bytes: policy,
                max_array_items: policy.map(|_| 256),
                ..Default::default()
            };
            let expected =
                saved_inspection::rows(&revision, &options, &program, declaration, source.path());
            let actual = profiles(
                options.max_string_bytes.unwrap_or(1),
                options.max_array_items.unwrap_or(1),
            )
            .into_iter()
            .map(|profile| {
                inspection::inspect(
                    &revision,
                    &options,
                    &program,
                    declaration,
                    source.path(),
                    profile,
                )
                .map(|facts| facts.render())
            })
            .collect::<Result<Vec<_>, _>>();
            assert_eq!(actual, expected, "field={field}, policy={policy:?}");
            if let Ok(rows) = actual {
                assert_eq!(rows.len(), 12);
                for offset in 0..12 {
                    let envelope = json!({"profiles":[],"offset":offset,"coverage":"continued","next_offset":Value::Null});
                    let expected = page(envelope.clone(), &rows, offset, 16384).unwrap();
                    assert_eq!(
                        paging::page(envelope, rows.len(), offset, expected.len(), |index| rows
                            [index]
                            .clone())
                        .unwrap(),
                        expected
                    );
                }
            }
        }
    }
}
