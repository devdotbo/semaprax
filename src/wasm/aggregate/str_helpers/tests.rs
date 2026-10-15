fn imports(source: &str) -> Vec<String> {
    let source = crate::check(source, "str-carrier-runtime.spx").unwrap();
    let bytes = crate::wasm::emit_module(&source).unwrap();
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    let mut names = Vec::new();
    for payload in wasmparser::Parser::new(0).parse_all(&bytes) {
        if let wasmparser::Payload::ImportSection(section) = payload.unwrap() {
            for import in section.into_imports() {
                names.push(import.unwrap().name.to_owned());
            }
        }
    }
    names
}
#[test]
fn owned_str_predicates_select_authenticated_string_carriers_not_raw_pointer_helpers() {
    let names = imports(
        r#"module app.owned_str;
@id("app.main") fn main()->i64 {let owner="nul\u{0}é";let value=string_as_str(owner);if str_starts_with(value,value)&&str_contains(value,value){1}else{0}}
"#,
    );
    for wanted in ["spx_string_starts_with_v1", "spx_string_contains_v1"] {
        assert_eq!(
            names.iter().filter(|name| name.as_str() == wanted).count(),
            1
        );
    }
}
#[test]
fn static_foreign_text_predicates_do_not_select_owned_string_runtime_authority() {
    let names = imports(
        r#"module app.static_str;
@id("app.main") fn main()->i64 {let value=str"text";if str_starts_with(value,value)&&str_contains(value,value){1}else{0}}
"#,
    );
    assert!(!names.iter().any(|name| name.starts_with("spx_string_")));
}
