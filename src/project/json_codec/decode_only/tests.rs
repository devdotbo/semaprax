use super::*;
const SCHEMA: &str = r#"module schema;
permit { process.stdin.read }
@id("app.row") record Row { @id("app.row.name") name:string, @id("app.row.n") n:i64, }
@id("app.request") record Request { @id("app.request.names") names:Vec<string>, @id("app.request.rows") rows:Vec<Row>, }
@id("schema.anchor") fn anchor()->i64 {0}
"#;
fn profiles() -> [(JsonCodecProfile, JsonCodecProfile); 4] {
    use JsonCodecProfile::*;
    [
        (OwnedRequest, OwnedRequestDecodeOnlyV1),
        (StreamOwnedRequest, StreamOwnedRequestDecodeOnlyV1),
        (
            Utf8OwnedRequest {
                max_string_bytes: 16,
            },
            Utf8OwnedRequestDecodeOnlyV1 {
                max_string_bytes: 16,
            },
        ),
        (
            StreamUtf8OwnedRequest {
                max_string_bytes: 16,
            },
            StreamUtf8OwnedRequestDecodeOnlyV1 {
                max_string_bytes: 16,
            },
        ),
    ]
}
#[test]
fn all_four_profiles_retain_exact_decoder_nodes_ids_literals_order_and_full_generators() {
    let program = crate::parse(SCHEMA, "schema.spx").unwrap();
    for (full, lean) in profiles() {
        let utf8 = matches!(
            full,
            JsonCodecProfile::Utf8OwnedRequest { .. }
                | JsonCodecProfile::StreamUtf8OwnedRequest { .. }
        );
        let expected = inventory(
            &program.types[1],
            &program.types[0],
            utf8,
            matches!(
                full,
                JsonCodecProfile::StreamOwnedRequest
                    | JsonCodecProfile::StreamUtf8OwnedRequest { .. }
            ),
        );
        assert_eq!(expected.functions.len(), if utf8 { 7 } else { 12 });
        assert_eq!(expected.imports.len(), if utf8 { 4 } else { 2 });
        let full_raw = super::super::profile_fragment(&program, &program.types[1], full).unwrap();
        // Entire old public generator remains the reference; this profile adds
        // no direction flag or branch inside any old renderer/template.
        let independent = match full {
            JsonCodecProfile::OwnedRequest => {
                super::super::owned::source(&program, &program.types[1], false)
            }
            JsonCodecProfile::StreamOwnedRequest => {
                super::super::owned::source(&program, &program.types[1], true)
            }
            JsonCodecProfile::Utf8OwnedRequest { max_string_bytes } => {
                super::super::utf8::source(&program, &program.types[1], max_string_bytes)
            }
            JsonCodecProfile::StreamUtf8OwnedRequest { max_string_bytes } => {
                super::super::utf8::stream_source(&program, &program.types[1], max_string_bytes)
            }
            _ => unreachable!(),
        }
        .unwrap();
        assert_eq!(full_raw, independent);
        let named_full =
            super::super::template::discard_bindings(&full_raw, SCHEMA, "schema.spx").unwrap();
        let mut reference = crate::parse(&named_full, "same.spx").unwrap();
        let projected = super::super::profile_fragment(&program, &program.types[1], lean).unwrap();
        let canonical_full =
            crate::format::canonical(&crate::parse(&named_full, "full.spx").unwrap());
        assert!(projected.len() < canonical_full.len());
        reference
            .functions
            .retain(|f| !expected.functions.contains_key(&f.stable_id));
        reference
            .types
            .retain(|t| t.stable_id != expected.variant_id);
        reference
            .module_uses
            .retain(|u| !expected.imports.contains(u.persistent_id.as_str()));
        // Canonical formatting covers every retained node, signature, ID,
        // literal byte and its order; physical source spans are revision-local.
        assert_eq!(projected, crate::format::canonical(&reference));
        assert_eq!(
            super::super::template::discard_bindings(&projected, SCHEMA, "schema.spx").unwrap(),
            projected
        );
        assert_eq!(
            projected,
            crate::format::canonical(&crate::parse(&projected, "roundtrip.spx").unwrap())
        );
        let lean_ast = crate::parse(&projected, "lean.spx").unwrap();
        assert_eq!(
            lean_ast.functions.len() + expected.functions.len(),
            crate::parse(&full_raw, "full.spx").unwrap().functions.len()
        );
        assert!(!lean_ast
            .types
            .iter()
            .any(|t| t.name == expected.variant_name));
        for function in &lean_ast.functions {
            assert!(!expected.functions.contains_key(&function.stable_id));
        }
        if matches!(
            full,
            JsonCodecProfile::StreamOwnedRequest | JsonCodecProfile::StreamUtf8OwnedRequest { .. }
        ) {
            assert!(projected.contains("json_Request_stream_normalize"));
        }
        eprintln!("OPT897 {full:?} -> {lean:?}: generated full={} projected={} removed_functions={} (source bytes only, no HIR/runtime gain claim)", canonical_full.len(), projected.len(), expected.functions.len());
    }
}
#[test]
fn missing_duplicate_forged_inventory_and_retained_dependencies_refuse_before_omission() {
    let program = crate::parse(SCHEMA, "schema.spx").unwrap();
    for (full, _) in profiles() {
        let utf8 = matches!(
            full,
            JsonCodecProfile::Utf8OwnedRequest { .. }
                | JsonCodecProfile::StreamUtf8OwnedRequest { .. }
        );
        let expected = inventory(
            &program.types[1],
            &program.types[0],
            utf8,
            matches!(
                full,
                JsonCodecProfile::StreamOwnedRequest
                    | JsonCodecProfile::StreamUtf8OwnedRequest { .. }
            ),
        );
        let raw = super::super::profile_fragment(&program, &program.types[1], full).unwrap();
        let id = expected.functions.keys().next().unwrap();
        let name = expected.functions[id].as_str();
        let decoder = if utf8 {
            "json_Request_utf8_owned_decode"
        } else {
            "json_Request_owned_decode"
        };
        for forged in [
            raw.replacen(&format!("@id(\"{id}\")"), "@id(\"foreign.encoder\")", 1),
            format!("{raw}\n@id(\"{id}\") fn counterfeit()->i64 {{0}}"),
            raw.replacen(name, "wrong_encoder_name", 1),
            raw.replace("as jv_i64_len;", "as wrong_alias;"),
            format!("{raw}\nuse function @id(\"foreign.import\") from foreign as unexpected;"),
            raw.replace("as jv_root;", "as wrong_retained_alias;"),
            raw.replacen(
                "let parsed=json_Request_request_decode(input);",
                "let parsed=json_Request_request_decode(input);let forbidden=jv_i64_len(0);",
                1,
            ),
            raw.replacen(
                &format!("fn {decoder}(input:borrow Slice<u8>)"),
                &format!(
                    "fn {decoder}(input:borrow Slice<u8>,forbidden:{})",
                    expected.variant_name
                ),
                1,
            ),
            raw.replacen(
                "name_start:usize",
                &format!("name_start:{}", expected.variant_name),
                1,
            ),
            format!(
                "{raw}\n@id(\"foreign.retained-call\") fn retained_call()->i64 {{jv_i64_len(0)}}"
            ),
            format!(
                "{raw}\n@id(\"foreign.retained-type\") fn retained_type(value:{})->i64 {{0}}",
                expected.variant_name
            ),
            raw.replace(".json.view-required\")", ".json.foreign-required\")"),
        ] {
            assert_ne!(forged, raw);
            assert_eq!(project(&forged, &expected).unwrap_err(), mismatch());
        }
    }
}
#[test]
fn selectors_and_plan_receipts_require_explicit_versions_and_exact_bounds() {
    for (_, profile) in profiles() {
        let (selector, bound, items) = super::super::plan::profile_fields(profile);
        assert!(selector.ends_with("decode-only.v1"));
        assert_eq!(items, None);
        let raw_bound = bound.map(|n| n.to_string());
        assert_eq!(
            super::super::parse_json_codec_profile(Some(selector), raw_bound.as_deref(), None),
            Some(profile)
        );
        let plan = format!("{{\"schema\":\"semaprax.json-codec-plan.v1\",\"entries\":[{{\"source\":\"src/schema.spx\",\"record\":\"app.request\",\"profile\":\"{selector}\"{}}}]}}", raw_bound.as_ref().map_or(String::new(), |b| format!(",\"max_string_bytes\":{b}")));
        let (_, entries) = super::super::decode_json_codec_plan(plan.as_bytes()).unwrap();
        assert_eq!(entries[0].profile, profile);
        assert!(super::super::parse_json_codec_profile(
            Some(selector),
            raw_bound.as_deref(),
            Some("1")
        )
        .is_none());
        if bound.is_some() {
            assert!(super::super::parse_json_codec_profile(Some(selector), None, None).is_none());
            for bad in ["0", "65", "01", "-1", "18446744073709551616"] {
                assert!(
                    super::super::parse_json_codec_profile(Some(selector), Some(bad), None)
                        .is_none()
                );
            }
        } else {
            assert!(
                super::super::parse_json_codec_profile(Some(selector), Some("1"), None).is_none()
            );
        }
        assert!(super::super::parse_json_codec_profile(
            Some(&selector.replace(".v1", ".v2")),
            raw_bound.as_deref(),
            None
        )
        .is_none());
    }
    assert_eq!(
        super::super::parse_json_codec_profile(None, None, None),
        Some(JsonCodecProfile::FlatScalars)
    );
}
#[test]
fn schema_stream_permit_and_utf8_bound_errors_remain_exact_full_profile_diagnostics() {
    let mut program = crate::parse(SCHEMA, "schema.spx").unwrap();
    program.permits.clear();
    for (full, lean) in profiles() {
        if matches!(
            full,
            JsonCodecProfile::StreamOwnedRequest | JsonCodecProfile::StreamUtf8OwnedRequest { .. }
        ) {
            assert_eq!(
                super::super::profile_fragment(&program, &program.types[1], full).unwrap_err(),
                super::super::profile_fragment(&program, &program.types[1], lean).unwrap_err()
            );
        }
    }
    for max_string_bytes in [0, 65, usize::MAX] {
        for (full, lean) in [
            (
                JsonCodecProfile::Utf8OwnedRequest { max_string_bytes },
                JsonCodecProfile::Utf8OwnedRequestDecodeOnlyV1 { max_string_bytes },
            ),
            (
                JsonCodecProfile::StreamUtf8OwnedRequest { max_string_bytes },
                JsonCodecProfile::StreamUtf8OwnedRequestDecodeOnlyV1 { max_string_bytes },
            ),
        ] {
            assert_eq!(
                super::super::profile_fragment(&program, &program.types[1], full).unwrap_err(),
                super::super::profile_fragment(&program, &program.types[1], lean).unwrap_err()
            );
        }
    }
    program.types[0].explicit_id = false;
    for (full, lean) in profiles() {
        assert_eq!(
            super::super::profile_fragment(&program, &program.types[1], full).unwrap_err(),
            super::super::profile_fragment(&program, &program.types[1], lean).unwrap_err()
        );
    }
}

#[test]
fn authored_discard_name_collisions_keep_the_full_fragment_decoder_bindings() {
    let authored =
        format!("{SCHEMA}\n@id(\"schema.collision\") fn json_discard_effect_0()->i64 {{0}}\n");
    let program = crate::parse(&authored, "schema.spx").unwrap();
    for (full, _) in profiles() {
        let utf8 = matches!(
            full,
            JsonCodecProfile::Utf8OwnedRequest { .. }
                | JsonCodecProfile::StreamUtf8OwnedRequest { .. }
        );
        let expected = inventory(
            &program.types[1],
            &program.types[0],
            utf8,
            matches!(
                full,
                JsonCodecProfile::StreamOwnedRequest
                    | JsonCodecProfile::StreamUtf8OwnedRequest { .. }
            ),
        );
        let raw = super::super::profile_fragment(&program, &program.types[1], full).unwrap();
        let named =
            super::super::template::discard_bindings(&raw, &authored, "schema.spx").unwrap();
        assert_eq!(
            source(&program, &program.types[1], full, &authored).unwrap(),
            project(&named, &expected).unwrap()
        );
        assert!(!named.contains("let json_discard_effect_0 ="));
    }
}
