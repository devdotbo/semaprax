use super::*;

const SCHEMA: &str = r#"module response;
@id("r.config") record Config {@id("r.config.label") label:string,@id("r.config.retry") retry:usize,}
@id("r.row") record Row {@id("r.row.sku") sku:string,@id("r.row.count") count:u8,}
@id("r.root") record Report {@id("r.root.config") config:Config,@id("r.root.rows") rows:Vec<Row>,@id("r.root.ok") ok:bool,}
@id("r.anchor") fn anchor()->i64{0}
"#;
fn generate(source: &str, strings: usize, rows: usize) -> Result<String, Vec<Diagnostic>> {
    let program = crate::parse(source, "schema.spx").unwrap();
    derive(&program, &program.types[2], strings, rows)
}

/// The generated bounded-String helper bodies, so a bound is checked in the
/// helper that owns it rather than anywhere in the output.
fn utf8_helper_bodies(output: &str) -> (&str, &str) {
    let body = |name: &str| {
        output
            .split(name)
            .nth(1)
            .unwrap()
            .split("@id(")
            .next()
            .unwrap()
    };
    (
        body("fn json_Report_nested_response_owned_valid("),
        body("fn json_Report_nested_response_utf8_quoted_len("),
    )
}

#[test]
fn nested_response_emits_recursive_borrowed_preflight_and_declaration_order() {
    let output = generate(SCHEMA, 16, 8).unwrap();
    assert_eq!(output, generate(SCHEMA, 16, 8).unwrap());
    assert_ne!(output, generate(SCHEMA, 15, 8).unwrap());
    assert_ne!(output, generate(SCHEMA, 16, 7).unwrap());
    assert!(output.contains("object_len_1(value:borrow Report)"));
    assert!(output.contains("object_len_4(values:borrow Vec<Row>,at:usize)"));
    assert!(output.contains("vec_field<Row>(values,at,\"sku\")"));
    assert!(!output.contains("vec_clone_at"));
    assert!(!output.contains("vec_into_iter"));
    assert!(output.contains("count<=8usize"));
    assert!(output.contains("ju_bounded_valid(bytes,16usize)"));
    assert!(output.contains("ju_quoted_len(bytes,16usize)"));
    let (owned_valid, quoted_len) = utf8_helper_bodies(&output);
    assert!(owned_valid.contains("ju_bounded_valid(bytes,16usize)"));
    assert!(quoted_len.contains("ju_quoted_len(bytes,16usize)"));
    let second = generate(SCHEMA, 15, 8).unwrap();
    assert!(second.contains("ju_bounded_valid(bytes,15usize)"));
    assert!(second.contains("ju_quoted_len(bytes,15usize)"));
    let (second_valid, second_quoted) = utf8_helper_bodies(&second);
    assert!(second_valid.contains("ju_bounded_valid(bytes,15usize)"));
    assert!(second_quoted.contains("ju_quoted_len(bytes,15usize)"));
    assert!(!second_valid.contains("16usize"));
    assert!(!second_quoted.contains("16usize"));
    assert!(output.contains("size>131072usize-total"));
    let api = output
        .split("fn json_Report_nested_response_encode(")
        .nth(1)
        .unwrap();
    assert!(api.find("required>output_limit").unwrap() < api.find("object_render_0").unwrap());
    assert!(
        api.find("required>output_limit").unwrap()
            < api
                .find("bytes_zeroed_bounded(required,131072usize)")
                .unwrap()
    );
    assert_eq!(output.matches("bytes_zeroed_bounded(").count(), 1);
    assert_eq!(output.matches("string_from_utf8(").count(), 1);
    // Construct directly into the owned field: an ordinary owning String
    // Place read can clone before the canonical aggregate field transfer.
    assert!(output.contains(
        "ReportJsonNestedResponseEncode::Encoded{text:string_from_utf8(bytes_as_slice(finished))}"
    ));
    assert!(!output.contains("let text=string_from_utf8("));
    assert!(!output.contains("string_concat"));
    assert!(!output.contains("string_from_i64"));
    assert!(!output.contains("string_from_usize"));
    let body = output
        .split("fn json_Report_nested_response_object_render_0")
        .nth(1)
        .unwrap();
    assert!(
        body.find("object_render_1(value,cursor,next)").unwrap() < body.find("value.rows").unwrap()
    );
    assert!(body.find("value.rows").unwrap() < body.find("value.ok").unwrap());
    let parsed = crate::parse(&output, "generated.spx").unwrap();
    assert!(parsed.functions.iter().all(|f| f.effects.is_empty()));
    assert!(parsed
        .functions
        .iter()
        .all(|f| f.stable_id.starts_with("r.root.json.nested-response.")));
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::parse(&canonical, "canonical.spx").unwrap())
    );
}

#[test]
fn nested_response_retains_copy_scalar_and_copy_record_vector_reads() {
    for ty in ["i64", "u8", "usize", "bool", "Row"] {
        let schema = SCHEMA
            .replace("sku:string", "sku:i64")
            .replace("Vec<Row>", &format!("Vec<{ty}>"));
        let output = generate(&schema, 16, 8).unwrap();
        assert!(output.contains(&format!("vec_get<{ty}>(values,at)")));
        assert!(!output.contains("vec_field"));
        if ty == "Row" {
            assert!(output.contains("object_len_4(value:Row)"));
        }
        crate::parse(&output, "copy.spx").unwrap();
    }
}

#[test]
fn nested_response_refuses_unborrowable_or_broadened_schema_before_generation() {
    for schema in [
        SCHEMA.replace("Vec<Row>", "Vec<string>"),
        SCHEMA
            .replace("ok:bool", "ok:Vec<i64>")
            .replace("retry:usize", "retry:Vec<u8>"),
        SCHEMA.replace("ok:bool", "ok:Report"),
        SCHEMA.replace("sku:string", "sku:Bytes"),
        SCHEMA.replace("@id(\"r.config.label\")", ""),
    ] {
        assert_eq!(generate(&schema, 16, 8).unwrap_err()[0].code, "SPX-J180");
    }
    for (strings, rows) in [(0, 8), (65, 8), (16, 0), (16, 257)] {
        assert_eq!(
            generate(SCHEMA, strings, rows).unwrap_err()[0].code,
            "SPX-J180"
        );
    }
    // The response route must not describe the frozen request policy. The
    // descriptor's request error remains exact, including its one-Vec boundary.
    let unsupported = SCHEMA.replace("ok:bool", "ok:char");
    let request_message = "nested request supports only explicit acyclic records, bounded Unicode Strings, i64/u8/usize/bool, and one admitted Vec";
    let response = generate(&unsupported, 16, 8).unwrap_err().remove(0);
    assert_eq!(response.code, "SPX-J180");
    assert_eq!(response.message, "nested response supports only explicit acyclic records, bounded Unicode Strings, i64/u8/usize/bool, and at most two admitted Vec fields");
    let program = crate::parse(&unsupported, "request-policy.spx").unwrap();
    assert_eq!(
        descriptor::validate(&program, &program.types[2], 16, 8)
            .err()
            .unwrap()[0]
            .message,
        request_message
    );
    let bounds = generate(SCHEMA, 0, 8).unwrap_err().remove(0);
    assert_eq!(bounds.code, "SPX-J180");
    assert!(bounds.message.starts_with("nested response "));
    let third = SCHEMA
        .replace("ok:bool", "ok:Vec<i64>")
        .replace("retry:usize", "retry:Vec<u8>");
    assert_eq!(
        generate(&third, 16, 8).unwrap_err()[0].message,
        "nested response admits at most two expanded Vec fields"
    );
    // Borrowed response inspection is independent of the decoder's allocation census.
    let two = SCHEMA.replace("count:u8", "count:string");
    assert!(generate(&two, 64, 256).is_ok());
    let program = crate::parse(&two, "request.spx").unwrap();
    assert_eq!(
        descriptor::validate(&program, &program.types[2], 64, 256)
            .err()
            .unwrap()[0]
            .code,
        "SPX-J180"
    );
    let sibling = two.replace("ok:bool", "ok:Vec<i64>");
    assert!(generate(&sibling, 64, 256).is_ok());
}

#[test]
fn nested_response_template_like_names_and_repeated_records_remain_opaque() {
    let schema = SCHEMA
        .replace("Report", "__ROW__")
        .replace("r.root", "r.__BOUND__.__ROW_ID__")
        .replace("ok:bool", "ok:Config");
    let output = generate(&schema, 16, 8).unwrap();
    let parsed = crate::parse(&output, "opaque.spx").unwrap();
    assert!(parsed.functions.iter().all(|f| f
        .stable_id
        .starts_with("r.__BOUND__.__ROW_ID__.json.nested-response.")));
    assert!(output.contains("object_len_7(value:borrow __ROW__)"));
    assert_eq!(
        parsed
            .functions
            .iter()
            .filter(|f| f.name.ends_with("writer_quote"))
            .count(),
        1
    );
}

fn generate_v2(source: &str, strings: usize, rows: usize) -> Result<String, Vec<Diagnostic>> {
    let program = crate::parse(source, "schema-v2.spx").unwrap();
    derive_v2(&program, &program.types[2], strings, rows)
}

#[test]
fn nested_response_v2_primitive_strings_use_named_borrowed_slots_and_distinct_identities() {
    let schema = SCHEMA.replace("Vec<Row>", "Vec<string>");
    assert_eq!(generate(&schema, 64, 256).unwrap_err()[0].code, "SPX-J180");
    let output = generate_v2(&schema, 64, 256).unwrap();
    assert_eq!(output, generate_v2(&schema, 64, 256).unwrap());
    assert!(output.contains("values:borrow Vec<string>"));
    assert!(output.contains("let value=vec_str_at(values,at);"));
    assert!(output.contains("json_Report_nested_response_v2_utf8_quoted_len(value)"));
    assert!(output.contains("json_Report_nested_response_v2_writer_quote(value,cursor,next)"));
    for absent in [
        "vec_get<string>",
        "vec_clone_at",
        "string_concat",
        "string_from_i64",
        "vec_str_at(value.rows",
    ] {
        assert!(!output.contains(absent), "{absent}");
    }
    assert_eq!(output.matches("bytes_zeroed_bounded(").count(), 1);
    assert_eq!(output.matches("string_from_utf8(").count(), 1);
    let api = output
        .split("fn json_Report_nested_response_v2_encode(")
        .nth(1)
        .unwrap();
    assert!(
        api.find("required>output_limit").unwrap() < api.find("bytes_zeroed_bounded(").unwrap()
    );
    assert!(output.contains("ReportJsonNestedResponseV2Encode::Encoded{text:string_from_utf8(bytes_as_slice(finished))}"));
    let parsed = crate::parse(&output, "v2.spx").unwrap();
    assert!(parsed
        .functions
        .iter()
        .all(|f| f.stable_id.starts_with("r.root.json.nested-response-v2.")));
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::parse(&canonical, "roundtrip.spx").unwrap())
    );
}

#[test]
fn nested_response_v2_preserves_two_path_bounds_and_frozen_request_policy() {
    let two = SCHEMA
        .replace("Vec<Row>", "Vec<string>")
        .replace("retry:usize", "retry:Vec<string>");
    let output = generate_v2(&two, 64, 256).unwrap();
    assert_eq!(
        output.matches("let value=vec_str_at(values,at);").count(),
        4
    );
    assert_eq!(output.matches("count<=256usize").count(), 2);
    assert!(output.contains("value.config.retry"));
    assert!(output.contains("value.rows"));
    let program = crate::parse(&two, "request-policy.spx").unwrap();
    assert_eq!(
        descriptor::validate(&program, &program.types[2], 64, 256)
            .err()
            .unwrap()[0]
            .code,
        "SPX-J180"
    );
    for (strings, items) in [(0, 256), (65, 256), (64, 0), (64, 257)] {
        assert_eq!(
            generate_v2(&two, strings, items).unwrap_err()[0].code,
            "SPX-J180"
        );
    }
    for schema in [
        two.replace("ok:bool", "ok:Vec<string>"),
        two.replace("ok:bool", "ok:Report"),
        two.replace("rows:Vec<string>", "rows:Vec<Vec<string>>"),
        two.replace("@id(\"r.config.label\")", ""),
    ] {
        assert_eq!(
            generate_v2(&schema, 64, 256).unwrap_err()[0].code,
            "SPX-J180"
        );
    }
    let scalar = generate(SCHEMA, 16, 8).unwrap();
    assert!(!scalar.contains("nested-response-v2"));
    let v2 = generate_v2(SCHEMA, 16, 8).unwrap();
    assert_ne!(v2, scalar);
}

#[test]
fn nested_response_v2_namespace_selection_keeps_authored_marker_names_opaque() {
    let schema = SCHEMA
        .replace("Vec<Row>", "Vec<string>")
        .replace("Report", "__ROW__nested_response_v2")
        .replace("r.root", "r.__BOUND__.nested-response-v2");
    let output = generate_v2(&schema, 16, 8).unwrap();
    let parsed = crate::parse(&output, "opaque-v2.spx").unwrap();
    assert!(parsed.functions.iter().all(|f| f
        .stable_id
        .starts_with("r.__BOUND__.nested-response-v2.json.nested-response-v2.")));
    assert!(output.contains("borrow __ROW__nested_response_v2"));
}
