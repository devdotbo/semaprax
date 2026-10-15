//! Complete 60-case corpus from PR #804 at d36d47daef42e5d7e06e16a7ebade20be66ba95e.
//! Authored source literals are preserved; old complete verdicts and intentional
//! scoped deltas are asserted separately. Independent exhaustiveness survives.
//!
//! Each case is a complete module with exactly one mistake an author brings
//! from another language. The verifier must name that mistake once: a result
//! it already refused has no type, and a name whose `let` it already refused
//! is not reported again as unknown. Every case pins the exact ordered codes,
//! so a new follow-on diagnostic fails here before it costs an edit cycle.

use std::path::Path;

use crate::{parse, verify};

const POINT: &str = "@id(\"p.point\")\nrecord Point {\n    @id(\"p.point.x\")\n    x: i64,\n    @id(\"p.point.y\")\n    y: i64,\n}\n\n";
const SHAPE: &str = "@id(\"p.shape\")\nvariant Shape {\n    @id(\"p.shape.dot\")\n    Dot,\n    @id(\"p.shape.box\")\n    Box {\n        @id(\"p.shape.box.w\")\n        w: i64,\n    },\n}\n\n";
const ADD: &str = "@id(\"p.add\")\nfn add(a: i64, b: i64) -> i64\n{\n    a + b\n}\n\n";
const MATCH_SHAPE: &str = "    match s { Shape::Dot {} => 0, Shape::Box { w: w } => w, }";

/// `main` with `body`, after the optional declarations in `prefix`.
fn main_with(prefix: &str, body: &str) -> String {
    format!("module probe.m;\n\n{prefix}@id(\"app.main\")\nfn main() -> i64\n{{\n{body}\n}}\n")
}

fn main(body: &str) -> String {
    main_with("", body)
}

/// One helper with `signature` and `body`, beside a trivial `main`.
fn helper(signature: &str, body: &str) -> String {
    format!(
        "module probe.m;\n\n@id(\"p.f\")\n{signature}\n{{\n{body}\n}}\n\n@id(\"app.main\")\nfn main() -> i64\n{{\n    0\n}}\n"
    )
}

fn codes(source: &str) -> Vec<&'static str> {
    let program = parse(source, Path::new("probe.spx")).expect("the grammar accepts this input");
    verify::verify(&program)
        .iter()
        .map(|diagnostic| diagnostic.code)
        .collect()
}

#[test]
fn full_proposal_corpus_keeps_exact_old_verdicts_and_scoped_intentional_deltas() {
    let cases: Vec<(&str, String, &str)> = vec![
        // Arithmetic over a string has no result type.
        ("string + string as a string result", helper("fn f(a: string, b: string) -> string", "    a + b"), "SPX-T250"),
        ("string + string bound, then used", main("    let s = \"a\" + \"b\";\n    string_len(s)"), "SPX-T250"),
        ("string + string as an argument", main("    string_len(\"a\" + \"b\")"), "SPX-T250"),
        ("string + string compared", main("    if \"a\" + \"b\" == \"ab\" { 7 } else { 8 }"), "SPX-T250"),
        ("string * integer as a string result", helper("fn f(a: string) -> string", "    a * 3"), "SPX-T250"),
        ("string + integer bound, then used", main("    let n = \"a\" + 1;\n    n + 1"), "SPX-T250"),
        ("string && bool", main("    if \"a\" && true { 1 } else { 0 }"), "SPX-T250"),
        ("string + string in a contract", helper("fn f(a: string) -> i64\n    requires a + a == \"xx\"", "    1"), "SPX-T250"),
        // An unknown function has no result type.
        ("unknown function bound, then used", main("    let x = abs(-3);\n    x + 1"), "SPX-T203"),
        ("unknown function as a string result", helper("fn f() -> string", "    to_upper(\"a\")"), "SPX-T203"),
        ("unknown function as an argument", main("    string_len(to_upper(\"a\"))"), "SPX-T203"),
        ("unknown function as a condition", main("    if is_even(4) { 1 } else { 0 }"), "SPX-T203"),
        ("method call on a string", main("    let text = \"abc\";\n    text.len()"), "SPX-T203"),
        // An unknown value has no type.
        ("unknown value in arithmetic", main("    let y = x + 1;\n    y"), "SPX-T202"),
        ("unknown value as a string result", helper("fn f() -> string", "    missing"), "SPX-T202"),
        ("unknown value as a condition", main("    if flag { 1 } else { 0 }"), "SPX-T202"),
        ("unknown value as an argument", main("    string_len(text)"), "SPX-T202"),
        ("unknown value in a contract", helper("fn f(a: i64) -> i64\n    requires limit > 0", "    a"), "SPX-T202"),
        // Scalar type mismatches.
        ("i32 + i64", main("    let a = 5i32;\n    let b = a + 1;\n    i64_from_i32(b)"), "SPX-T208"),
        ("usize < i64", main("    let n = 3usize;\n    if n < 5 { 1 } else { 0 }"), "SPX-T208"),
        ("f64 + i64", main("    let x = 1.5 + 2;\n    i64_from_f64(x)"), "SPX-T208"),
        ("i64 && i64", main("    if 1 && 2 { 1 } else { 0 }"), "SPX-T208"),
        ("bool + i64", main("    let x = true + 1;\n    x"), "SPX-T208"),
        ("comparison + i64", main("    let x = (1 < 2) + 1;\n    x"), "SPX-T208"),
        ("! on i64", main("    if !5 { 1 } else { 0 }"), "SPX-T206"),
        ("- on bool bound, then used", main("    let b = -true;\n    if b { 1 } else { 0 }"), "SPX-T206"),
        ("i64 as an if condition", main("    if 1 { 1 } else { 0 }"), "SPX-T210"),
        ("call result as an if condition", main_with(ADD, "    if add(1, 2) { 1 } else { 0 }"), "SPX-T210"),
        ("i64 as a while condition", main("    let mut n = 3;\n    while n {\n        n = n - 1;\n        0\n    }\n    n"), "SPX-T251"),
        ("if branches of different types", main("    let x = if true { 1 } else { \"a\" };\n    x"), "SPX-T211"),
        ("i64 index for vec_get", main("    let values = vec_of<i64>(1, 2);\n    vec_get<i64>(values, 0)"), "SPX-T205"),
        // Results that differ from the signature.
        ("i64 result for a bool function", helper("fn f() -> bool", "    1"), "SPX-T103"),
        ("i64 result for a string function", helper("fn f() -> string", "    1"), "SPX-T103"),
        // Calls.
        ("too few arguments", main_with(ADD, "    add(1) + 1"), "SPX-T204"),
        ("too many arguments", main_with(ADD, "    add(1, 2, 3) + 1"), "SPX-T204"),
        ("wrong argument type", main_with(ADD, "    add(\"a\", 2) + 1"), "SPX-T205"),
        ("missing element type argument", main("    let values = vec_of<i64>(1, 2);\n    i64_from_usize(vec_len(values))"), "SPX-T281"),
        // Records.
        ("unknown field read", main_with(POINT, "    let p = Point { x: 1, y: 2 };\n    p.z + 1"), "SPX-T214"),
        ("missing field", main_with(POINT, "    let p = Point { x: 1 };\n    p.x"), "SPX-T213"),
        ("unknown record type bound, then used", main("    let p = Pont { x: 1, y: 2 };\n    p.x + 1"), "SPX-T001"),
        ("field of the wrong type", main_with(POINT, "    let p = Point { x: true, y: 2 };\n    p.x + p.y"), "SPX-T215"),
        ("extra field", main_with(POINT, "    let p = Point { x: 1, y: 2, z: 3 };\n    p.x"), "SPX-T212"),
        ("field read on a scalar", main("    let n = 5;\n    n.value + 1"), "SPX-T214"),
        // Variants and matches.
        ("misspelled case", main_with(SHAPE, &format!("    let s = Shape::Circle {{}};\n{MATCH_SHAPE}")), "SPX-T215"),
        ("lower-case Option case", main("    let o = Option<i64>::none {};\n    match o { Option::Some { value: v } => v, Option::None {} => 0, }"), "SPX-T215"),
        ("payload of the wrong type", main_with(SHAPE, &format!("    let s = Shape::Box {{ w: true }};\n{MATCH_SHAPE}")), "SPX-T215"),
        ("missing match case", main_with(SHAPE, "    let s = Shape::Dot {};\n    match s { Shape::Dot {} => 0, }"), "SPX-M101"),
        ("match arms of different types", main("    let n = 2;\n    let x = match n { 0 => 1, _ => true, };\n    x"), "SPX-T259"),
        ("scalar match without a catch-all", main("    let n = 2;\n    match n { 0 => 1, 1 => 2, }"), "SPX-T257"),
        ("unknown case in a match pattern", main_with(SHAPE, "    let s = Shape::Dot {};\n    match s { Shape::Dot {} => 0, Shape::Square { w: w } => w, }"), "SPX-M103"),
        // Signatures that name an unknown type.
        ("unknown parameter type", helper("fn f(a: int) -> i64", "    a + 1"), "SPX-T001"),
        ("unknown result type", helper("fn f(a: i64) -> int", "    a + 1"), "SPX-T001"),
        // Bindings.
        ("assignment to an immutable binding", main("    let x = 1;\n    x = x + 1;\n    x"), "SPX-U101"),
        ("shadowing", main("    let x = 1;\n    let x = x + 1;\n    x"), "SPX-T209"),
        ("declared bool binding given an i64", main("    let x: bool = 1;\n    if x { 1 } else { 0 }"), "SPX-T232"),
        ("assignment of the wrong type", main("    let mut x = 1;\n    x = true;\n    x"), "SPX-U102"),
        ("unknown declared binding type", main("    let x: int = 1;\n    x + 1"), "SPX-T232"),
        // Ownership and contracts.
        ("use after move", main("    let a = \"x\";\n    let b = \"y\";\n    let c = string_concat(a, b);\n    string_len(c) + string_len(a)"), "SPX-O101"),
        ("double move", helper("fn f(a: string) -> string", "    string_concat(a, a)"), "SPX-O101"),
        ("non-bool ensures", helper("fn f(a: i64) -> i64\n    ensures result", "    a"), "SPX-C101"),
    ];
    assert_eq!(cases.len(), 60);
    for (mistake, source, code) in cases {
        let program = parse(&source, Path::new("probe.spx")).unwrap();
        let old = crate::source_verify::declaration::old::verify(&program);
        let current = crate::source_verify::verify(&program);
        let expected_old: &[&str] = match mistake {
            "string + string as a string result" | "string * integer as a string result" => {
                &["SPX-T250", "SPX-T103", "SPX-O104"]
            }
            "string + string bound, then used" | "string + string as an argument" => {
                &["SPX-T250", "SPX-T205"]
            }
            "string + string compared" | "string + string in a contract" => {
                &["SPX-T250", "SPX-T207"]
            }
            "unknown function bound, then used" => &["SPX-T203", "SPX-T202"],
            "- on bool bound, then used" => &["SPX-T206", "SPX-T210"],
            "i64 result for a string function" => &["SPX-T103", "SPX-O104"],
            "unknown record type bound, then used" => {
                &["SPX-T001", "SPX-T215", "SPX-T212", "SPX-T212", "SPX-T202"]
            }
            "unknown case in a match pattern" => &["SPX-M103", "SPX-T202", "SPX-M101"],
            "unknown parameter type" => &["SPX-T001", "SPX-T208"],
            "unknown result type" => &["SPX-T001", "SPX-T103"],
            "declared bool binding given an i64" => &["SPX-T232", "SPX-T210"],
            _ => &[code],
        };
        assert_eq!(super::codes(&old), expected_old, "old {mistake}:\n{source}");
        let expected_current: &[&str] = if mistake == "unknown case in a match pattern" {
            &["SPX-M103", "SPX-M101"]
        } else {
            &[code]
        };
        assert_eq!(
            super::codes(&current),
            expected_current,
            "current {mistake}:\n{source}"
        );
        assert_eq!(current.first(), old.first(), "first refusal: {mistake}");
        let mut remaining = old.iter();
        for diagnostic in &current {
            assert!(
                remaining.any(|original| original == diagnostic),
                "full original diagnostic: {mistake}: {diagnostic:?}"
            );
        }
    }
}

#[test]
fn refused_patterns_and_signatures_excuse_only_their_own_names() {
    // A complete match still reports the unknown value in an arm it admits.
    assert_eq!(
        codes(&main_with(
            SHAPE,
            "    let s = Shape::Dot {};\n    match s { Shape::Dot {} => missing, Shape::Box { w: w } => w, }"
        )),
        ["SPX-T202"]
    );
    // A refused pattern excuses the names it binds, not other unknown names.
    assert_eq!(
        codes(&main_with(
            SHAPE,
            "    let s = Shape::Dot {};\n    match s { Shape::Dot {} => 0, Shape::Square { w: w } => w + missing, }"
        )),
        ["SPX-M103", "SPX-T202", "SPX-M101"]
    );
    // Coverage is still judged when every pattern is admitted.
    assert_eq!(
        codes(&main_with(
            SHAPE,
            "    let s = Shape::Dot {};\n    match s { Shape::Dot {} => 0, }"
        )),
        ["SPX-M101"]
    );
    // An unknown parameter type excuses that parameter, not the others.
    assert_eq!(
        codes(&helper("fn f(a: int, b: bool) -> i64", "    a + b")),
        ["SPX-T001", "SPX-T208"]
    );
    // An ownership-mode error leaves a known parameter type in force.
    assert_eq!(
        codes(&helper("fn f(a: own i64) -> bool", "    a + 1")),
        ["SPX-O002", "SPX-T103"]
    );
    // A type that exists but is refused keeps later refusals of its uses.
    assert_eq!(
        codes(&helper(
            "fn f(a: Option<Option<i64>>) -> Option<bool>",
            "    let inner = a?;
    Option<bool>::None {}"
        )),
        ["SPX-T223", "SPX-T218"]
    );
    // A known result type is still compared with the body.
    assert_eq!(
        codes(&helper("fn f(a: int) -> bool", "    1")),
        ["SPX-T001", "SPX-T103"]
    );
}

#[test]
fn an_unknown_value_is_still_reported_when_no_refused_let_declares_it() {
    // Read before the `let` that declares it.
    assert_eq!(
        codes(&main("    let y = x + 1;\n    let x = abs(-3);\n    y")),
        ["SPX-T202", "SPX-T203"]
    );
    // Read inside the declaring initializer.
    assert_eq!(codes(&main("    let x = x + 1;\n    x")), ["SPX-T202"]);
    // Declared only inside a sibling block that has already ended.
    assert_eq!(
        codes(&main(
            "    let a = if true { let x = abs(-3); 1 } else { 0 };\n    a + x"
        )),
        ["SPX-T203", "SPX-T202"]
    );
    // A refused `let` of another name does not excuse this one.
    assert_eq!(
        codes(&main("    let x = abs(-3);\n    y + 1")),
        ["SPX-T203", "SPX-T202"]
    );
    // A successful `let` binds normally; a later unknown name is unrelated.
    assert_eq!(codes(&main("    let x = 1;\n    x + y")), ["SPX-T202"]);
}
