//! Intentional wrong-field recovery controls for SCOPED-DIAGNOSTIC-RECOVERY-V1.
//!
//! The recursive oracle and the saved legacy iterative receiver still report a
//! read of a refused pattern binding; the current receiver intentionally does
//! not. This module owns the full old/current expectations for the one pinned
//! corpus fixture and bounded semantic controls for its neighbours. The only
//! intentional expectations name complete Diagnostic objects at exact authored
//! read sites in `assert_recovered`; every other diagnostic must match in full.
use super::receive_route;
use crate::ast::Span;
use crate::diagnostic::Diagnostic;

const PATH: &str = "scoped-recovery.spx";
const SCALAR_PATH: &str = "iterative-verifier.spx";
const PUBLIC_PATH: &str = "constructor-full-public.spx";
const FIXTURE_BODY: &str = "match (Option<i64>::Some { value: 1 }) { Option::Some { wrong: n } => n, Option::None {} => 0, }";
const FIXTURE_LITERAL: &str = "\"match (Option<i64>::Some { value: 1 }) { Option::Some { wrong: n } => n, Option::None {} => 0, }\"";

/// Full-byte pin of the original failing fixture as the corpus formats it.
pub(in crate::source_verify) const PINNED: &str = "module t; @id(\"t.main\") fn main()->i64 { while false { match (Option<i64>::Some { value: 1 }) { Option::Some { wrong: n } => n, Option::None {} => 0, } } 0 }";

pub(in crate::source_verify) fn is_pinned(source: &str) -> bool {
    source == PINNED
}

fn wrong_field(path: &str) -> Diagnostic {
    Diagnostic::error(
        "SPX-M104",
        "unknown or duplicate pattern field `wrong` in `Option::Some`",
        Span {
            start: 111,
            end: 119,
            line: 1,
            column: 112,
        },
    )
    .at_path(path)
}

fn missing_value(path: &str) -> Diagnostic {
    Diagnostic::error(
        "SPX-M104",
        "pattern `Option::Some` is missing payload field `value`",
        Span {
            start: 96,
            end: 121,
            line: 1,
            column: 97,
        },
    )
    .at_path(path)
}

fn dependent_read(path: &str) -> Diagnostic {
    Diagnostic::error(
        "SPX-T202",
        "unknown value `n` in `main`",
        Span {
            start: 125,
            end: 126,
            line: 1,
            column: 126,
        },
    )
    .at_path(path)
}

/// Recursive oracle and saved legacy iterative: exactly three, in this order.
fn expected_old(path: &str) -> Vec<Diagnostic> {
    vec![wrong_field(path), missing_value(path), dependent_read(path)]
}

/// Current iterative: the two original M104 refusals only.
fn expected_current(path: &str) -> Vec<Diagnostic> {
    vec![wrong_field(path), missing_value(path)]
}

fn assert_pinned_spans() {
    assert_eq!(
        PINNED,
        format!(
            "module t; @id(\"t.main\") fn main()->i64 {{ while false {{ {FIXTURE_BODY} }} 0 }}"
        )
    );
    assert_eq!(&PINNED[111..119], "wrong: n");
    assert_eq!(&PINNED[96..121], "Option::Some { wrong: n }");
    assert_eq!(&PINNED[125..126], "n");
}

/// Recursive, complete saved legacy iterative and current iterative receivers
/// on the pinned fixture, each with full six-field expectations.
fn assert_all_receivers(source: &str) {
    assert_eq!(source, PINNED);
    let old = expected_old(PATH);
    let current = expected_current(PATH);
    let (recursive_value, recursive) = receive_route(source, 2);
    let (legacy_value, legacy) = receive_route(source, 1);
    let (current_value, actual) = receive_route(source, 0);
    assert_eq!(recursive, old);
    assert_eq!(legacy, old);
    assert_eq!(actual, current);
    assert_eq!(actual.as_slice(), &legacy[..2]);
    assert_eq!(recursive_value, legacy_value);
    assert_eq!(legacy_value, current_value);
}

/// Receiving adapter for the scalar corpus helpers: oracle vs iterative.
pub(in crate::source_verify) fn assert_scalar(
    source: &str,
    oracle: &[Diagnostic],
    iterative: &[Diagnostic],
) {
    assert_pinned_spans();
    assert_eq!(oracle, expected_old(SCALAR_PATH).as_slice());
    assert_eq!(iterative, expected_current(SCALAR_PATH).as_slice());
    assert_all_receivers(source);
}

/// Receiving adapter for the public pass: current vs saved old declaration pass.
pub(in crate::source_verify) fn assert_public(
    source: &str,
    current: &[Diagnostic],
    old: &[Diagnostic],
) {
    assert_eq!(source, PINNED);
    assert_pinned_spans();
    let expected_current = expected_current(PUBLIC_PATH);
    let expected_old = expected_old(PUBLIC_PATH);
    assert_eq!(current, expected_current.as_slice());
    assert_eq!(old, expected_old.as_slice());
    assert_eq!(
        current.iter().map(Diagnostic::json).collect::<Vec<_>>(),
        expected_current
            .iter()
            .map(Diagnostic::json)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        old.iter().map(Diagnostic::json).collect::<Vec<_>>(),
        expected_old
            .iter()
            .map(Diagnostic::json)
            .collect::<Vec<_>>()
    );
    assert_eq!(
        current.first().map(Diagnostic::json),
        old.first().map(Diagnostic::json)
    );
}

fn diagnostics(source: &str, receiver: u8) -> Vec<Diagnostic> {
    receive_route(source, receiver).1
}

fn codes(diagnostics: &[Diagnostic]) -> Vec<&'static str> {
    diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

/// Ordinary, loop and traversal contexts around one `match x { .. }` body.
fn contexts(body: &str) -> [String; 3] {
    [
        format!("module t; @id(\"t.main\") fn main()->i64 {{ let x=Option<i64>::Some {{ value: 1 }}; {body} }}"),
        format!("module t; @id(\"t.main\") fn main()->i64 {{ while false {{ let x=Option<i64>::Some {{ value: 1 }}; {body} }} 0 }}"),
        format!("module t; @id(\"t.main\") fn main()->i64 {{ let values=vec_with_capacity<i64>(1usize); for item in values {{ let x=Option<i64>::Some {{ value: 1 }}; {body} }} 0 }}"),
    ]
}

/// Each intentional omitted diagnostic is independently authored at one exact
/// source site, including all six fields. Neither receiver is normalized.
fn assert_recovered(
    source: &str,
    sites: &[(&str, usize, &str)],
) -> (Vec<Diagnostic>, Vec<Diagnostic>) {
    let (recursive_value, recursive) = receive_route(source, 2);
    let (legacy_value, legacy) = receive_route(source, 1);
    let (current_value, current) = receive_route(source, 0);
    assert_eq!(legacy, recursive, "{source}");
    assert_eq!(legacy_value, recursive_value, "{source}");
    assert_eq!(current_value, legacy_value, "{source}");
    let mut expected = legacy.clone();
    let mut previous = None;
    for &(literal, offset, name) in sites {
        assert_eq!(source.matches(literal).count(), 1, "{source}: {literal}");
        let start = source.find(literal).unwrap() + offset;
        let end = start + name.len();
        assert_eq!(&source[start..end], name);
        let omitted = Diagnostic::error(
            "SPX-T202",
            format!("unknown value `{name}` in `main`"),
            Span {
                start,
                end,
                line: 1,
                column: start + 1,
            },
        )
        .at_path(PATH);
        assert_eq!(legacy.iter().filter(|value| **value == omitted).count(), 1);
        let ordinal = legacy.iter().position(|value| *value == omitted).unwrap();
        assert!(previous.is_none_or(|before| before < ordinal));
        previous = Some(ordinal);
        let index = expected.iter().position(|value| *value == omitted).unwrap();
        expected.remove(index);
    }
    assert_eq!(current, expected, "{source}");
    (legacy, current)
}

/// Receivers that recovery must not touch: legacy equals current in full
/// (value and diagnostics) and equals the recursive oracle's diagnostics.
fn assert_unchanged(source: &str) -> Vec<Diagnostic> {
    let recursive = diagnostics(source, 2);
    let legacy = receive_route(source, 1);
    let current = receive_route(source, 0);
    assert_eq!(legacy.1, recursive, "{source}");
    assert_eq!(legacy, current, "{source}");
    current.1
}

fn check_unknown_field(
    body: &str,
    sites: &[(&str, usize, &str)],
    old_codes: &[&str],
    current_codes: &[&str],
) {
    for (index, source) in contexts(body).iter().enumerate() {
        let (legacy, current) = assert_recovered(source, sites);
        assert!(legacy.len() > current.len(), "{source}");
        if index < 2 && !old_codes.is_empty() {
            assert_eq!(codes(&legacy), old_codes, "{source}");
            assert_eq!(codes(&current), current_codes, "{source}");
        }
    }
}

#[test]
fn pinned_fixture_has_exact_bytes_spans_and_full_old_current_diagnostics() {
    assert_pinned_spans();
    assert_all_receivers(PINNED);
    let old = expected_old(PATH);
    assert_eq!(old[0].code, "SPX-M104");
    assert_eq!(old[1].code, "SPX-M104");
    assert_eq!(old[2].code, "SPX-T202");
    assert!(old.iter().all(|diagnostic| diagnostic.help.is_none()));
}

#[test]
fn original_failing_fixture_literals_stay_in_both_original_corpora() {
    for corpus in [
        include_str!("../iterative_verifier_tests.rs"),
        include_str!("../constructor_tests/corpus.rs"),
    ] {
        assert_eq!(corpus.matches(FIXTURE_LITERAL).count(), 1);
    }
}

#[test]
fn unknown_field_refusals_drop_only_exact_dependent_reads_in_every_context() {
    const M104: &str = "SPX-M104";
    const T202: &str = "SPX-T202";
    check_unknown_field(
        "match x { Option::Some { wrong: n } => n, Option::None {} => 0, }",
        &[("=> n", 3, "n")],
        &[M104, M104, T202],
        &[M104, M104],
    );
    check_unknown_field(
        "match x { Option::Some { wrong: n, wrong: m } => n + m, Option::None {} => 0, }",
        &[("=> n", 3, "n"), ("n + m", 4, "m")],
        &[M104, M104, M104, T202, T202],
        &[M104, M104, M104],
    );
    check_unknown_field(
        "match x { Option::Some { wrong: n } => n + zzz, Option::None {} => 0, }",
        &[("=> n", 3, "n")],
        &[M104, M104, T202, T202],
        &[M104, M104, T202],
    );
    check_unknown_field(
        "match x { Option::Some { wrong: n } if n > 0 => n, Option::None {} => 0, }",
        &[(" if n > 0", 4, "n"), ("=> n", 3, "n")],
        &[],
        &[],
    );
    check_unknown_field(
        "match x { Option::Some { wrong: n } => n, Option::None {} => n, }",
        &[("wrong: n } => n", 14, "n")],
        &[M104, M104, T202, T202],
        &[M104, M104, T202],
    );
    check_unknown_field(
        "let bad = missing; match x { Option::Some { wrong: n } => n + bad, Option::None {} => 0, }",
        &[("=> n", 3, "n"), ("n + bad", 4, "bad")],
        &[T202, M104, M104, T202, T202],
        &[T202, M104, M104],
    );
    check_unknown_field(
        "match x { Other::Some { value: n } => n, Option::None {} => 0, }",
        &[("=> n", 3, "n")],
        &["SPX-M103", T202, "SPX-M101"],
        &["SPX-M103", "SPX-M101"],
    );
}

#[test]
fn recovery_retains_independent_identifier_and_sibling_arm_reads() {
    let independent =
        contexts("match x { Option::Some { wrong: n } => n + zzz, Option::None {} => 0, }");
    for source in &independent[..2] {
        let current = diagnostics(source, 0);
        assert_eq!(
            current.last().unwrap().message,
            "unknown value `zzz` in `main`"
        );
    }
    let sibling = contexts("match x { Option::Some { wrong: n } => n, Option::None {} => n, }");
    for source in &sibling[..2] {
        let legacy = diagnostics(source, 1);
        let current = diagnostics(source, 0);
        assert_eq!(legacy.len(), 4, "{source}");
        assert_eq!(current.len(), 3, "{source}");
        assert_eq!(current[2], legacy[3], "{source}");
        assert_ne!(current[2].span, legacy[2].span, "{source}");
        let span = current[2].span.unwrap();
        assert_eq!(&source[span.start..span.end], "n");
    }
}

#[test]
fn valid_duplicate_field_and_duplicate_binding_names_stay_unchanged() {
    for (index, source) in
        contexts("match x { Option::Some { value: n } => n, Option::None {} => 0, }")
            .iter()
            .enumerate()
    {
        let found = assert_unchanged(source);
        if index < 2 {
            assert!(found.is_empty(), "{source}: {found:?}");
        }
    }
    for (index, source) in
        contexts("match x { Option::Some { value: n, value: m } => n + m, Option::None {} => 0, }")
            .iter()
            .enumerate()
    {
        let found = assert_unchanged(source);
        if index < 2 {
            assert_eq!(codes(&found), ["SPX-M104"], "{source}");
            let span = found[0].span.unwrap();
            assert_eq!(&source[span.start..span.end], "value: m");
        }
    }
    for (index, source) in
        contexts("match x { Option::Some { value: result } => 0, Option::None {} => 0, }")
            .iter()
            .enumerate()
    {
        let found = assert_unchanged(source);
        if index < 2 {
            assert_eq!(codes(&found), ["SPX-M104"], "{source}");
            assert_eq!(
                found[0].message,
                "invalid or duplicate pattern binding `result`"
            );
        }
    }
    for (index, source) in
        contexts("let y=1; match x { Option::Some { value: y } => y, Option::None {} => 0, }")
            .iter()
            .enumerate()
    {
        let found = assert_unchanged(source);
        if index < 2 {
            assert_eq!(codes(&found), ["SPX-M104"], "{source}");
            assert_eq!(found[0].message, "invalid or duplicate pattern binding `y`");
        }
    }
    for (index, source) in
        contexts("match x { Option::Some { value: n } => n + zzz, Option::None {} => 0, }")
            .iter()
            .enumerate()
    {
        let found = assert_unchanged(source);
        if index < 2 {
            assert_eq!(codes(&found), ["SPX-T202"], "{source}");
        }
    }
}

#[test]
fn closure_reads_of_a_refused_pattern_binding_keep_original_errors() {
    let source = contexts(
        "match x { Option::Some { wrong: n } => { let f = fn() -> i64 { n }; 0 }, Option::None {} => 0, }",
    )[0]
    .clone();
    let legacy = receive_route(&source, 1);
    let current = receive_route(&source, 0);
    assert_eq!(legacy, current, "{source}");
    assert!(legacy.1.len() >= 3, "{source}: {:?}", legacy.1);
    assert_eq!(codes(&legacy.1)[..2], ["SPX-M104", "SPX-M104"]);
}
