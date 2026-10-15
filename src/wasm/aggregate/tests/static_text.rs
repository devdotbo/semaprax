//! Exact selected pool bounds, independent of source or guest input authority.
use super::super::OwnedUtf8Literals;

#[test]
fn selected_literal_pool_exact_bound_empty_and_one_over_are_truthful() {
    let mut pool = OwnedUtf8Literals {
        static_pool: true,
        ..Default::default()
    };
    let full = "a".repeat(65_536);
    let base = pool.base();
    assert_eq!(pool.intern(&full).unwrap(), (base, 65_536));
    assert_eq!(pool.intern("").unwrap(), (base, 0));
    assert_eq!(pool.bytes.len(), 65_536);
    assert_eq!(pool.intern(&full).unwrap(), (base, 65_536));
    assert!(pool.intern("b").is_err());
    assert_eq!(pool.bytes.len(), 65_536);
    let mut legacy = OwnedUtf8Literals::default();
    assert_eq!(legacy.intern(&full).unwrap(), (base, 65_536));
    assert_eq!(legacy.intern("").unwrap(), (base + 65_536, 0));
}

#[test]
fn selected_static_text_search_helpers_are_indexed_in_the_internal_module() {
    let source = r#"module static.search;
@id("search") fn search(text:borrow str)->bool {
    str_starts_with(text,str"é") && str_contains(text,str"\u{0}")
}
@id("main") fn main()->i64 {if search(str"é\u{0}"){42}else{0}}
"#;
    let program = crate::hir::resolve(&crate::parse(source, "static-search.spx").unwrap()).unwrap();
    let bytes = crate::wasm::emit_resolved_module(&program).unwrap();
    assert_eq!(bytes, crate::wasm::emit_resolved_module(&program).unwrap());
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
}

#[test]
fn selected_static_text_search_does_not_grant_public_scalar_or_text_exports() {
    let source = r#"module static.public;
@id("length") fn length(text:borrow str)->i64 {str_len_bytes(text)}
@id("scalar") fn scalar()->i64 {42}
@id("main") fn main()->i64 {length(str"é")}
"#;
    let program = crate::hir::resolve(&crate::parse(source, "static-public.spx").unwrap()).unwrap();
    assert_eq!(
        crate::wasm::emit_resolved_module_with_scalar_exports(&program, &["scalar".to_owned()])
            .unwrap_err()
            .code,
        "SPX-W119"
    );
    assert_eq!(
        crate::wasm::emit_resolved_module_with_text_exports(&program, &["length".to_owned()])
            .unwrap_err()
            .code,
        "SPX-W119"
    );
}
