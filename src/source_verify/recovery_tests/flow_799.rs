//! Conditional and match refusal provenance; full old receivers remain executable.
use super::{codes, receive_route};

const PREFIX: &str = "module t; @id(\"choice\") variant Choice { @id(\"none\") None, @id(\"some\") Some { @id(\"value\") value: i64, }, } ";
fn with_choice(body: &str) -> String {
    format!("{PREFIX} fn main(choice: Choice) -> i64 {{ {body} }}")
}

#[test]
fn refused_conditions_and_incompatible_branches_never_publish_checked_results() {
    for (body, expected) in [
        ("if missing { 1 } else { 0 }", vec!["SPX-T202"]),
        ("if 1 { 1 } else { 0 }", vec!["SPX-T210"]),
        ("if true { 1 } else { false }", vec!["SPX-T211"]),
        (
            "if missing { let x = 1; x } else { let y = 0; y }",
            vec!["SPX-T202"],
        ),
    ] {
        let source = super::source(body);
        let old = receive_route(&source, 3);
        let current = receive_route(&source, 0);
        assert!(old.0.is_some(), "{body}");
        assert!(current.0.is_none(), "{body}");
        assert_eq!(codes(&current.1), expected);
        assert_eq!(current.1, old.1);
    }
}

#[test]
fn nested_conditional_refusal_preserves_both_branch_errors_and_outer_sibling() {
    let source = super::source("(if 1 { missing_then } else { missing_else }) + missing_sibling");
    let old = receive_route(&source, 3);
    let current = receive_route(&source, 0);
    assert_eq!(
        codes(&current.1),
        ["SPX-T210", "SPX-T202", "SPX-T202", "SPX-T202"]
    );
    assert_eq!(current.1, old.1);
    assert!(current.0.is_none());
}

#[test]
fn unknown_match_scrutinee_supplies_no_pattern_type_and_keeps_independent_body_error() {
    let source = with_choice(
        "match missing { Choice::Some { value: item } => item, Choice::None {} => independent, }",
    );
    let old = receive_route(&source, 3);
    let current = receive_route(&source, 0);
    assert_eq!(codes(&current.1), ["SPX-T202", "SPX-T202"]);
    assert!(current.0.is_none());
    assert_eq!(current.1[0], old.1[0]);
    assert_eq!(current.1[1], *old.1.last().unwrap());
    assert!(old.1.iter().any(|diagnostic| diagnostic.code == "SPX-M103"));
}

#[test]
fn unknown_match_keeps_independent_guard_errors_and_refused_names_stay_arm_local() {
    let source = with_choice(
        "match missing { Choice::Some { value: item } if item > 0 => item, _ => item, }",
    );
    let current = receive_route(&source, 0);
    assert_eq!(codes(&current.1), ["SPX-T202", "SPX-T202"]);
    assert!(current.0.is_none());
    let source = with_choice(
        "match missing { Choice::Some { value: item } if independent_guard => item, _ => 0, }",
    );
    let current = receive_route(&source, 0);
    assert_eq!(codes(&current.1), ["SPX-T202", "SPX-T202"]);
}

#[test]
fn unknown_match_keeps_real_duplicate_fields_bindings_and_outer_shadowing() {
    let source = with_choice("let item = 1; match missing { Choice::Some { value: item, value: duplicate } => 0, _ => 0, }");
    let current = receive_route(&source, 0);
    assert_eq!(codes(&current.1), ["SPX-T202", "SPX-M104", "SPX-M104"]);
    assert!(current.0.is_none());
    let source = with_choice("match missing { Choice::Some { value: item } => item, Choice::Some { value: other } => other, _ => 0, }");
    let current = receive_route(&source, 0);
    assert_eq!(codes(&current.1), ["SPX-T202", "SPX-M102"]);
}

#[test]
fn known_match_missing_case_and_admitted_conditional_receivers_remain_exact() {
    let source = with_choice("match choice { Choice::None {} => 0, }");
    let old = receive_route(&source, 3);
    let current = receive_route(&source, 0);
    assert_eq!(codes(&current.1), ["SPX-M101"]);
    assert_eq!(current, old);
    for body in [
        "if true { 1 } else { 0 }",
        "if false { let x = 1; x } else { let y = 0; y }",
        "if true { 1 } else { 0 } + 2",
    ] {
        let source = super::source(body);
        let old = receive_route(&source, 3);
        assert!(old.1.is_empty());
        assert_eq!(receive_route(&source, 0), old);
    }
}

#[test]
fn complete_matching_and_arm_receivers_have_exact_full_source_inverses() {
    let mut matching = include_str!("../iterative/matching/matching_before_799.rs")
        .replace("before_match_799_", "");
    matching = matching.replace("            refused_input: None,\n", "");
    assert_eq!(
        matching,
        include_str!(
            "../../../docs/evidence/conditional-refusal-799/iterative__matching.rs.before"
        )
    );
    let arms =
        include_str!("../iterative/match_arms/arms_before_799.rs").replace("before_match_799_", "");
    assert_eq!(
        arms,
        include_str!(
            "../../../docs/evidence/conditional-refusal-799/iterative__match_arms.rs.before"
        )
    );
}

#[test]
fn read_only_scrutinee_origin_is_frozen_before_later_pattern_and_body_errors() {
    let mut recovery = crate::source_verify::recovery::Recovery::default();
    recovery.fork(None, 0);
    let initial = recovery.checkpoint(&[]);
    let mut diagnostics = vec![crate::diagnostic::Diagnostic::io_static(
        "SPX-T202",
        "root refusal",
    )];
    recovery.refused_let(0, "refused", initial, &diagnostics);
    let scrutinee = recovery.checkpoint(&diagnostics);
    assert!(recovery.reads_refused(0, "refused"));
    let origin = recovery.refusal_origin(scrutinee, &diagnostics).unwrap();
    assert_eq!(origin, 0);
    diagnostics.push(crate::diagnostic::Diagnostic::io_static(
        "SPX-M104",
        "later independent pattern error",
    ));
    recovery.fork(Some(0), 1);
    recovery.refused_origin(1, "item", origin);
    let item = recovery.checkpoint(&diagnostics);
    assert!(recovery.reads_refused(1, "item"));
    assert_eq!(recovery.refusal_origin(item, &diagnostics), Some(0));
}

#[test]
fn unknown_match_keeps_independent_aggregate_body_refusal_without_scalar_unreachability() {
    let source = with_choice("match missing { _ => \"body\", }");
    let old = receive_route(&source, 3);
    let current = receive_route(&source, 0);
    assert!(current.0.is_none());
    assert_eq!(current.1[0], old.1[0]);
    assert_eq!(current.1[1], *old.1.last().unwrap());
    assert_eq!(current.1.len(), 2);
    let source = with_choice("match missing { 1 => 0, 1 => 0, _ => 0, _ => 0, }");
    let current = receive_route(&source, 0);
    assert_eq!(codes(&current.1), ["SPX-T202"]);
    assert!(current.0.is_none());
}
