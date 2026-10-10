use super::*;
const SOURCE: &str = include_str!("../../../../tests/fixtures/record-traversal-maximum.spx");
fn parsed(source: &str) -> Program {
    crate::parse(source, "fusion-candidate.spx").unwrap()
}

#[test]
fn original_maximum_inventory_is_exact_borrowed_source_and_remains_refused() {
    let program = parsed(SOURCE);
    let proof = inventory(&program).expect("finite source candidate, no admission");
    assert!(std::ptr::eq(proof.function, &program.functions[0]));
    assert!(std::ptr::eq(proof.record, &program.types[0]));
    assert_eq!(
        (
            proof.count,
            proof.width,
            proof.payload_bytes,
            proof.string_bytes
        ),
        (4096, 3, 12288, 12288)
    );
    assert!(matches!(proof.byte_site.kind, K::Call { .. }));
    let errors = crate::check(SOURCE, "fusion-candidate.spx").unwrap_err();
    let refusal = errors
        .iter()
        .find(|e| e.code == "SPX-T267")
        .expect("unchanged source allocation refusal");
    let help = refusal
        .help
        .as_deref()
        .expect("specific unsupported-profile guidance");
    assert!(help.contains("4096 Bytes-producing executions"));
    assert!(help.contains("source recognition is not fusion or partition authority"));
    assert_eq!(crate::byte_data_capacity::MAX_BYTES_COPY_SITES, 32);
    let (help, overflowed, used) =
        crate::bounded_output::with_limit_usage(0, || refusal_help(&program));
    assert!(
        help.is_none(),
        "optional context requires its bounded scratch"
    );
    assert!(
        !overflowed,
        "optional help must not replace the selected refusal"
    );
    assert_eq!(used, 0);
}

#[test]
fn candidate_counts_and_widths_are_source_derived_without_fixture_constants() {
    for (count, width) in [(1, 1), (32, 5), (127, 7), (4096, 3)] {
        let source = SOURCE.replace("4096", &count.to_string()).replace(
            "bytes_zeroed(3usize)",
            &format!("bytes_zeroed({width}usize)"),
        );
        let program = parsed(&source);
        let proof = inventory(&program).unwrap();
        assert_eq!((proof.count, proof.width), (count, width));
        assert_eq!(proof.payload_bytes, count * width);
        assert_eq!(proof.string_bytes, count * 3);
    }
    let exact_backing = SOURCE.replace("bytes_zeroed(3usize)", "bytes_zeroed(32usize)");
    assert_eq!(
        inventory(&parsed(&exact_backing)).unwrap().payload_bytes,
        crate::byte_ops::MAX_OWNED_BYTE_VALUE_BYTES
    );
    for source in [
        SOURCE.replace("4096", "4097"),
        SOURCE.replace("4096", "0"),
        SOURCE.replace("bytes_zeroed(3usize)", "bytes_zeroed(33usize)"),
    ] {
        assert!(inventory(&parsed(&source)).is_none());
    }
    // Source names/identities are not a selector; exact binding scopes are.
    let renamed = SOURCE
        .replace("items", "construction")
        .replace("next", "index_before")
        .replace("app.main", "different.entry");
    assert!(inventory(&parsed(&renamed)).is_some());
}

#[test]
fn forged_trip_count_constructor_escape_and_shape_drift_have_no_candidate() {
    for (from, to) in [
        ("while next < 4096", "while next < 4095"),
        ("next = next + 1", "next = next + 2"),
        (
            "payload: bytes_zeroed(3usize)",
            "payload: bytes_clone(bytes_zeroed(3usize))",
        ),
        (
            "items = vec_push<Row>(items, item);",
            "let escaped = item; items = vec_push<Row>(items, item);",
        ),
        ("title: \"é\\u{0}\"", "title: string_clone(\"é\\u{0}\")"),
        ("app.leaf.row.payload", "app.leaf.row.title"),
        (
            "bytes_zeroed(3usize)",
            "bytes_zeroed(18446744073709551615usize)",
        ),
    ] {
        let source = SOURCE.replace(from, to);
        assert_ne!(source, SOURCE, "hostile mutation must be real: {from}");
        assert!(inventory(&parsed(&source)).is_none(), "{from} -> {to}");
    }
}

#[test]
fn unsupported_clone_escape_shadow_and_read_after_transfer_close_consumer_domain() {
    for (from, to) in [
        (
            "let rows = items;",
            "let rows = items; let cloned = vec_clone_at<Row>(rows, 0usize);",
        ),
        (
            "let rows = items;",
            "let rows = items; let dead = vec_clear<Row>(rows);",
        ),
        (
            "let rows = items;",
            "let rows = items; let consumed = vec_sort_owned<Row>(rows);",
        ),
        ("let rows = items;", "let rows = items; let items = 0;"),
        (
            "let utf8 = str_as_bytes(text);",
            "let utf8 = foreign_read(text);",
        ),
        ("total = total + row.priority;", "total = row.title;"),
    ] {
        let source = SOURCE.replace(from, to);
        assert_ne!(source, SOURCE);
        assert!(inventory(&parsed(&source)).is_none(), "{from} -> {to}");
    }
    let locals: String = (0..80)
        .map(|index| format!("let extra_{index}=0; "))
        .collect();
    let flat = SOURCE.replace("let rows = items;", &format!("let rows = items; {locals}"));
    assert!(
        inventory(&parsed(&flat)).is_none(),
        "bounded borrowed scope refuses before growth"
    );
}
