use super::*;

const SCHEMA: &str = r#"module schema;
@id("response.row") record Row {
 @id("response.row.number") number:i64,
 @id("response.row.label") label:string,
 @id("response.row.active") active:bool,
}
@id("response.metrics") record Metrics {@id("response.metrics.count") count:usize,}
@id("response.root") record Report {
 @id("response.root.metrics") stats:Metrics,
 @id("response.root.items") entries:Vec<Row>,
}
@id("schema.anchor") fn anchor()->i64{0}
"#;

const MULTI_STRING_SCHEMA: &str = r#"module schema;
@id("response.item") record Item {
 @id("response.item.id") id:string,
 @id("response.item.server") server:string,
 @id("response.item.arrival") arrival:i64,
 @id("response.item.start") start:i64,
 @id("response.item.finish") finish:i64,
 @id("response.item.wait") wait:i64,
 @id("response.item.late") late:bool,
 @id("response.item.ordinal") ordinal:i64,
}
@id("response.metrics") record Metrics {@id("response.metrics.count") count:usize,}
@id("response.root") record Report {
 @id("response.root.items") entries:Vec<Item>,
 @id("response.root.metrics") stats:Metrics,
}
@id("schema.anchor") fn anchor()->i64{0}
"#;

const ROW_SOURCE: &str = r#"@id("response.row.json.collection-response.object-len")
fn json_Row_response_object_len(value:borrow Row)->usize {
if !(json_Row_response_owned_valid(string_as_str(value.label))){18446744073709551615usize}else{30usize+(usize_from_i64(jv_i64_len(value.number)))+(json_Row_response_utf8_quoted_len(string_as_str(value.label)))+(if value.active{4usize}else{5usize})}}
@id("response.row.json.collection-response.object-render")
fn json_Row_response_object_render(value:borrow Row,position:usize,output:own Bytes)->Bytes
requires json_Row_response_object_len(value)!=18446744073709551615usize && position<=byte_len(bytes_as_slice(output)) && json_Row_response_object_len(value)<=byte_len(bytes_as_slice(output))-position {
let mut next=output;let mut cursor=position;
next=bytes_set(next,cursor,123u8);cursor=cursor+1usize;
let k0={let k=[34u8,110u8,117u8,109u8,98u8,101u8,114u8,34u8,58u8];next=json_Row_response_writer_bytes(array_as_slice(k),cursor,next);0};cursor=cursor+9usize;
next=json_Row_response_writer_i64(value.number,cursor,next);cursor=cursor+(usize_from_i64(jv_i64_len(value.number)));
let k1={let k=[44u8,34u8,108u8,97u8,98u8,101u8,108u8,34u8,58u8];next=json_Row_response_writer_bytes(array_as_slice(k),cursor,next);0};cursor=cursor+9usize;
next=json_Row_response_writer_quote(string_as_str(value.label),cursor,next);cursor=cursor+(json_Row_response_utf8_quoted_len(string_as_str(value.label)));
let k2={let k=[44u8,34u8,97u8,99u8,116u8,105u8,118u8,101u8,34u8,58u8];next=json_Row_response_writer_bytes(array_as_slice(k),cursor,next);0};cursor=cursor+10usize;
next=json_Row_response_writer_bool(value.active,cursor,next);cursor=cursor+(if value.active{4usize}else{5usize});
next=bytes_set(next,cursor,125u8);cursor=cursor+1usize;
next}
"#;

#[test]
fn collection_response_is_closed_ordered_and_canonical_without_source_authority() {
    let program = crate::parse(SCHEMA, "schema.spx").unwrap();
    let root = &program.types[2];
    let output = source(&program, root, 64).unwrap();
    assert_eq!(output, source(&program, root, 64).unwrap());
    assert_ne!(output, source(&program, root, 63).unwrap());
    assert!(output.contains("vec_len<Row>(value.entries)"));
    assert!(!output.contains("vec_clone_at"));
    assert!(output.contains("json_Row_response_object_len_at(value.entries,at)"));
    assert!(output.contains("json_Row_response_object_render_at(value.entries,at,cursor,output)"));
    assert!(output.contains("fn json_Metrics_response_object_len(value:Metrics)"));
    assert!(output.contains("fn json_Row_response_object_len(value:borrow Row)"));
    assert!(output.contains("string_as_str(value.label)"));
    assert!(output.contains("json_Row_response_utf8_quoted_len(string_as_str(value.label))"));
    assert!(
        output.contains("json_Row_response_writer_quote(string_as_str(value.label),cursor,next)")
    );
    assert!(!output.contains("match borrow value"));
    assert!(!output.contains("response_field_"));
    // Independent exact projection contract: no borrow-match wrapper or synthetic
    // String binding, and identical validity/length/render ordering.
    assert_eq!(
        record::source(&program.types[0], &program.types[0]),
        ROW_SOURCE
    );
    // Authored fields can resemble helper locals; direct roots must retain the
    // actual declaration field, not accidentally read a generated local.
    let field_names = SCHEMA
        .replace("number:i64", "value:i64")
        .replace("label:string", "output_0:string")
        .replace("active:bool", "response_field_1:bool");
    let renamed_fields = crate::parse(&field_names, "field-names.spx").unwrap();
    let field_output = record::source(&renamed_fields.types[0], &renamed_fields.types[0]);
    assert!(field_output.contains("string_as_str(value.output_0)"));
    assert!(field_output.contains("jv_i64_len(value.value)"));
    assert!(field_output.contains("if value.response_field_1"));
    let field_generated = source(&renamed_fields, &renamed_fields.types[2], 64).unwrap();
    let field_parsed = crate::parse(&field_generated, "field-projection.spx").unwrap();
    let field_canonical = crate::format::canonical(&field_parsed);
    assert_eq!(
        field_canonical,
        crate::format::canonical(&crate::parse(&field_canonical, "field-canonical.spx").unwrap())
    );
    // Full generator canonical round-trip below remains the authority-neutral
    // source gate; the declaration field assertions exercise the renamed shape.
    assert!(output.contains("count<=256usize"));
    // The same byte ceiling is now enforced by the authenticated shared kernel.
    assert!(output.contains("ju_bounded_valid(bytes,64usize)"));
    assert!(output.contains("ju_quoted_len(bytes,64usize)"));
    let smaller = source(&program, root, 63).unwrap();
    assert!(smaller.contains("ju_bounded_valid(bytes,63usize)"));
    assert!(smaller.contains("ju_quoted_len(bytes,63usize)"));
    let kernel = include_str!("../../../../std/data-json-utf8/src/utf8.spx");
    assert!(kernel.contains("if max_bytes > 64usize { false }"));
    assert!(kernel.contains("if length > max_bytes { false }"));
    assert!(kernel.contains("if !bounded_valid(input, max_bytes) { 18446744073709551615usize }"));
    assert!(output.contains("required>output_limit"));
    assert_eq!(output.matches("bytes_zeroed_bounded(").count(), 1);
    assert_eq!(output.matches("string_from_utf8(").count(), 1);
    // Construct directly into the owned field: an ordinary owning String
    // Place read can clone before the canonical aggregate field transfer.
    assert!(output.contains("ReportJsonCollectionResponseEncode::Encoded{text:string_from_utf8(bytes_as_slice(output))}"));
    assert!(!output.contains("let text=string_from_utf8("));
    assert!(!output.contains("string_concat"));
    assert!(!output.contains("string_from_i64"));
    assert!(!output.contains("string_from_usize"));
    assert!(output.contains("size>131072usize-total-comma"));
    assert!(!output.contains("ju_escape_scalar"));
    assert!(!output.contains("vec_sort"));
    assert!(!output.contains("stdin"));
    let parsed = crate::parse(&output, "response.spx").unwrap();
    // Imported library identities retain their exact owning namespaces. Only
    // generated declarations belong to this response profile's namespace.
    assert!(parsed
        .module_uses
        .iter()
        .all(|import| { import.kind == crate::ast::ModuleUseKind::Function }));
    let imports: Vec<_> = parsed
        .module_uses
        .iter()
        .map(|import| {
            (
                import.persistent_id.as_str(),
                import.target_module.as_str(),
                import.alias.as_str(),
            )
        })
        .collect();
    assert_eq!(
        imports,
        vec![
            (
                "std.data.json.digits.i64_len",
                "std.data.json.digits",
                "jv_i64_len"
            ),
            (
                "std.data.json.write.usize_len",
                "std.data.json.write",
                "jv_usize_len"
            ),
            (
                "std.data.json.utf8.scalar_at",
                "std.data.json.utf8",
                "ju_raw_scalar"
            ),
            (
                "std.data.json.utf8.sequence_end",
                "std.data.json.utf8",
                "ju_raw_end"
            ),
            (
                "std.data.json.utf8.bounded_valid",
                "std.data.json.utf8",
                "ju_bounded_valid"
            ),
            (
                "std.data.json.utf8.quoted_len",
                "std.data.json.utf8",
                "ju_quoted_len"
            ),
        ]
    );
    for function in &parsed.functions {
        assert!(!function.stable_id.contains(".json.utf8."));
        assert!(function.stable_id.starts_with("response."));
        assert!(function.stable_id.contains(".json.collection-response."));
    }
    for declaration in &parsed.types {
        assert!(!declaration.stable_id.contains(".json.utf8."));
        assert!(declaration.stable_id.contains(".json.collection-response."));
        let TypeDeclarationKind::Variant { cases } = &declaration.kind else {
            panic!("response-generated variant")
        };
        for case in cases {
            assert!(!case.stable_id.contains(".json.utf8."));
            assert!(case.stable_id.contains(".json.collection-response."));
            for field in &case.fields {
                assert!(!field.stable_id.contains(".json.utf8."));
                assert!(field.stable_id.contains(".json.collection-response."));
            }
        }
    }
    assert!(parsed.permits.is_empty());
    assert!(parsed.functions.iter().all(|f| f.effects.is_empty()));
    let encode = parsed
        .functions
        .iter()
        .find(|f| f.name == "json_Report_collection_response_encode")
        .unwrap();
    assert!(encode
        .stable_id
        .ends_with(".json.collection-response.encode"));
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::parse(&canonical, "canonical.spx").unwrap())
    );
    assert!(canonical.len() < super::super::MAX_GENERATED_BYTES);
    // Source field/type names can resemble generated templates. Substitution
    // must only rewrite the template, never authored identity bytes.
    let renamed = SCHEMA
        .replace("Row", "json_Test_utf8_Row")
        .replace("response.row", "response.json.utf8.row");
    let program = crate::parse(&renamed, "renamed.spx").unwrap();
    let output = source(&program, &program.types[2], 8).unwrap();
    assert!(output.contains("response.json.utf8.row.json.collection-response.owned-valid"));
    assert!(output.contains("json_json_Test_utf8_Row_response_owned_valid"));
}

#[test]
fn collection_response_refuses_wrong_bound_nested_shapes_and_unchecked_identities() {
    for bound in [0, 65, usize::MAX] {
        let p = crate::parse(SCHEMA, "schema.spx").unwrap();
        assert_eq!(
            source(&p, &p.types[2], bound).unwrap_err()[0].code,
            "SPX-J180"
        );
    }
    for schema in [
        SCHEMA.replace("stats:Metrics", "stats:Vec<Row>"),
        SCHEMA.replace("entries:Vec<Row>", "entries:Row"),
        SCHEMA.replace("label:string", "label:Bytes"),
        SCHEMA.replace("count:usize", "count:string"),
        SCHEMA.replace("number:i64", "number:i32"),
        SCHEMA.replace("@id(\"response.row.label\")", ""),
        SCHEMA.replace("record Row", "record Row<T>"),
    ] {
        let p = crate::parse(&schema, "bad.spx").unwrap();
        assert_eq!(source(&p, &p.types[2], 64).unwrap_err()[0].code, "SPX-J180");
    }
    let mut p = crate::parse(SCHEMA, "schema.spx").unwrap();
    let invariant = crate::parse("module x;fn main()->bool{false}", "x.spx")
        .unwrap()
        .functions
        .remove(0)
        .body;
    p.types[1].invariants = Some(Box::new(vec![invariant]));
    assert_eq!(source(&p, &p.types[2], 64).unwrap_err()[0].code, "SPX-J180");
}

#[test]
fn collection_response_checks_and_encodes_each_bounded_string_in_row_order() {
    let program = crate::parse(MULTI_STRING_SCHEMA, "multi-string-schema.spx").unwrap();
    let output = source(&program, &program.types[2], 64).unwrap();
    let row = record::source(&program.types[0], &program.types[0]);
    let first_check =
        "let response_string_valid_0=json_Item_response_owned_valid(string_as_str(value.id));";
    let second_check =
        "let response_string_valid_1=json_Item_response_owned_valid(string_as_str(value.server));";
    assert!(row.contains(first_check));
    assert!(row.contains(second_check));
    assert!(row.contains("if !(response_string_valid_0&&response_string_valid_1)"));
    assert!(row.contains("json_Item_response_utf8_quoted_len(string_as_str(value.id))"));
    assert!(row.contains("json_Item_response_utf8_quoted_len(string_as_str(value.server))"));
    assert!(row.contains("json_Item_response_writer_quote(string_as_str(value.id),cursor,next)"));
    assert!(
        row.contains("json_Item_response_writer_quote(string_as_str(value.server),cursor,next)")
    );
    assert!(row.contains("let k=[34u8,105u8,100u8,34u8,58u8]"));
    assert!(row.contains("let k=[44u8,34u8,115u8,101u8,114u8,118u8,101u8,114u8,34u8,58u8]"));
    assert!(output.contains("vec_len<Item>(value.entries)"));
    let parsed = crate::parse(&output, "multi-string-generated.spx").unwrap();
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(
        canonical,
        crate::format::canonical(&crate::parse(&canonical, "multi-string-canonical.spx").unwrap())
    );

    let too_many_strings = MULTI_STRING_SCHEMA.replace(
        "@id(\"response.item.server\") server:string,",
        "@id(\"response.item.server\") server:string,\n @id(\"response.item.region\") region:string,",
    );
    let malformed = crate::parse(&too_many_strings, "too-many-strings.spx").unwrap();
    assert_eq!(
        source(&malformed, &malformed.types[2], 64).unwrap_err()[0].code,
        "SPX-J180"
    );
}

#[test]
fn collection_response_scoped_rows_keep_direct_helpers_and_exact_field_selectors() {
    let schema = MULTI_STRING_SCHEMA
        .replace("id:string", "values:string")
        .replace("server:string", "at:string")
        .replace("arrival:i64", "output_0:i64");
    let program = crate::parse(&schema, "scoped-row-fields.spx").unwrap();
    let row = &program.types[0];
    let indexed = record::indexed_source(row, row);
    assert!(
        indexed.contains("fn json_Item_response_object_len_at(values:borrow Vec<Item>,at:usize)")
    );
    assert!(indexed
        .contains("fn json_Item_response_object_render_at(values:borrow Vec<Item>,at:usize,position:usize,output:own Bytes)"));
    assert!(indexed.contains("response.item.json.collection-response.object-len-at"));
    assert!(indexed.contains("response.item.json.collection-response.object-render-at"));
    for field in fields(row) {
        assert!(
            indexed.contains(&format!("vec_field<Item>(values,at,\"{}\")", field.name)),
            "{}",
            field.name
        );
    }
    let first = indexed.find("let response_string_valid_0=json_Item_response_owned_valid(vec_field<Item>(values,at,\"values\"));").unwrap();
    let second = indexed.find("let response_string_valid_1=json_Item_response_owned_valid(vec_field<Item>(values,at,\"at\"));").unwrap();
    assert!(first < second);
    assert!(!indexed.contains("string_as_str"));
    assert!(!indexed.contains("vec_clone_at"));
    let output = source(&program, &program.types[2], 64).unwrap();
    // Keep the original direct Row helpers byte-for-byte while the root's two
    // passes choose only independently authenticated indexed reads.
    assert!(output.contains(&record::source(row, row)));
    assert!(output.contains(&indexed));
    assert!(!output.contains("vec_clone_at"));
    let parsed = crate::parse(&output, "scoped-response.spx").unwrap();
    let canonical = crate::format::canonical(&parsed);
    assert_eq!(
        canonical,
        crate::format::canonical(
            &crate::parse(&canonical, "scoped-response-canonical.spx").unwrap()
        )
    );
    assert!(canonical.len() < super::super::MAX_GENERATED_BYTES);
}
