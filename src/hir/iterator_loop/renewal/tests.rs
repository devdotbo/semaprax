//! Lexical loops preserve the existing exact record-owner renewal classifier.
use super::*;
#[test]
fn lexical_continue_keeps_record_renewal_inside_its_actual_loop() {
    let source = r#"module record.renewal;
@id("cursor") record Cursor { @id("cursor.bytes") bytes: Bytes, @id("cursor.index") index: usize, }
@id("advance") fn advance(value: Cursor)->Cursor { value }
@id("main") fn main()->i64 {
 let mut cursor=Cursor{bytes:bytes_zeroed(0usize),index:0usize};
 while false { cursor=advance(cursor); continue; }
 0
}"#;
    let parsed = crate::check(source, "lexical-record-renewal.spx").unwrap();
    let program = crate::hir::resolve(&parsed).unwrap();
    crate::hir::validate(&program).unwrap();
    let function = program
        .functions
        .iter()
        .find(|function| function.id.as_str() == "main")
        .unwrap();
    assert!(function_may_require_record_renewal(function));
    assert!(function_requires_record_renewal(&program, function));
    assert!(!function_has_record_renewal_outside_loop(
        &program, function
    ));
    assert_eq!(renewal_bindings(&program, function).len(), 1);
}
