//! Full saved operator receiver beside current refusal propagation.
use super::{codes, receive_route, source};

#[test]
fn refused_operators_keep_all_original_primary_fields_and_publish_no_checked_type() {
    for (body, expected) in [
        ("missing + 1", vec!["SPX-T202"]),
        ("missing + also_missing", vec!["SPX-T202", "SPX-T202"]),
        ("true + 1", vec!["SPX-T208"]),
        ("1 == true", vec!["SPX-T207"]),
        ("missing == also_missing", vec!["SPX-T202", "SPX-T202"]),
        ("true && missing", vec!["SPX-T202"]),
        ("false || missing", vec!["SPX-T202"]),
    ] {
        let source = source(body);
        let old = receive_route(&source, 3);
        let current = receive_route(&source, 0);
        assert!(old.0.is_some(), "{body}");
        assert!(current.0.is_none(), "{body}");
        assert_eq!(codes(&current.1), expected, "{body}");
        assert_eq!(current.1, old.1, "every diagnostic field: {body}");
    }
}

#[test]
fn nested_refusal_removes_only_dependent_condition_and_argument_classifications() {
    for (body, old_codes, current_codes) in [
        (
            "if missing + 1 { 1 } else { 0 }",
            vec!["SPX-T202", "SPX-T210"],
            vec!["SPX-T202"],
        ),
        (
            "string_len(true + 1)",
            vec!["SPX-T208", "SPX-T205"],
            vec!["SPX-T208"],
        ),
        (
            "if missing + 1 { missing_then } else { missing_else }",
            vec!["SPX-T202", "SPX-T210", "SPX-T202", "SPX-T202"],
            vec!["SPX-T202", "SPX-T202", "SPX-T202"],
        ),
    ] {
        let source = source(body);
        let old = receive_route(&source, 3);
        let current = receive_route(&source, 0);
        assert_eq!(codes(&old.1), old_codes, "{body}");
        assert_eq!(codes(&current.1), current_codes, "{body}");
        let mut expected = old.1.clone();
        expected.remove(1);
        assert_eq!(
            current.1, expected,
            "all retained diagnostics remain exact: {body}"
        );
    }
}

#[test]
fn refused_initializer_keeps_real_duplicate_and_independent_sibling_errors() {
    let source = source("let value = true + 1; let value = missing; value");
    let old = receive_route(&source, 3);
    let current = receive_route(&source, 0);
    assert_eq!(codes(&old.1), ["SPX-T208", "SPX-T202", "SPX-T209"]);
    assert_eq!(current.1, old.1);
    assert!(old.0.is_some());
    assert!(current.0.is_none());
    for body in [
        "let refused = true + 1; missing",
        "let x = x + 1; x",
        "{ let x = true + 1; x }; x",
    ] {
        let source = source(body);
        let current = receive_route(&source, 0);
        assert!(current
            .1
            .iter()
            .any(|diagnostic| diagnostic.code == "SPX-T202"));
        assert!(current.0.is_none());
    }
}

#[test]
fn admitted_operator_results_and_lazy_owned_call_joins_match_full_saved_receiver() {
    for body in [
        "1 + 2 * 3",
        "-1",
        "!false",
        "true && false",
        "false || true",
        "1 == 1",
        "1 < 2",
        "let value = 1 + 2; value",
    ] {
        let source = source(body);
        let old = receive_route(&source, 3);
        let current = receive_route(&source, 0);
        assert!(old.1.is_empty(), "{body}");
        assert_eq!(current, old, "{body}");
    }
    let source = "module t; @id(\"t.buffer\") resource Buffer { @id(\"t.buffer.drop\") drop trivial; } fn inspect(value: borrow Buffer) -> i64 { 1 } fn consume(value: own Buffer) -> bool { true } fn main(buffer: own Buffer, left: bool, right: bool) -> i64 { let selected = left && (right && consume(buffer)); inspect(buffer) }";
    let old = receive_route(source, 3);
    let current = receive_route(source, 0);
    assert!(!old.1.is_empty());
    assert_eq!(current, old);
}

#[test]
fn complete_original_operator_source_is_preserved_by_exact_named_inverse() {
    let mut original = include_str!("../iterative/operators/before_799.rs").to_owned();
    for name in [
        "frame_resume_unary",
        "frame_resume_binary_left",
        "frame_resume_binary_right",
        "frame_resume_if_condition",
        "frame_resume_if_then",
        "frame_resume_if_else",
    ] {
        original = original.replace(&format!("fn before_799_{name}("), &format!("fn {name}("));
    }
    assert_eq!(original, include_str!("../../../docs/evidence/operator-refusal-799/src__source_verify__iterative__operators.rs.before"));
}
