//! Actual CLI projection keeps each complete old subsection and fails exact misses.
use super::*;
const ORIGINAL: &[(&str, &str)] = &[
    ("records", include_str!("language_details/records.md")),
    ("io", include_str!("language_details/io.md")),
    ("boxes", include_str!("language_details/boxes.md")),
    (
        "record-traversal",
        include_str!("language_details/record-traversal.md"),
    ),
    (
        "field-views",
        include_str!("language_details/field-views.md"),
    ),
    ("imports", include_str!("language_details/imports.md")),
];
#[test]
fn exact_language_details_preserve_all_complete_original_cli_sections() {
    for (slug, expected) in ORIGINAL {
        let query = format!("detail:{slug}");
        let (output, directory) = invoke(&["help", "language", &query]);
        assert!(output.status.success(), "{query}: {output:?}");
        assert!(output.stderr.is_empty());
        assert_eq!(output.stdout, expected.as_bytes());
        assert!(output.stdout.len() <= 2_048);
        std::fs::remove_dir(directory).unwrap();
    }
    for query in [
        "detail:",
        "detail:Records",
        "detail:record",
        "detail:records:0",
        "detail:all",
        "detail:topics",
    ] {
        let (output, directory) = invoke(&["help", "language", query]);
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        std::fs::remove_dir(directory).unwrap();
    }
}
#[test]
fn bounded_topics_name_exact_single_detail_queries_and_keep_full_reference() {
    for (topic, slugs) in [
        ("records", &["records", "imports"][..]),
        ("strings", &["io"][..]),
        ("builtins", &["boxes"][..]),
        ("lists", &["record-traversal"][..]),
        ("projects", &["field-views"][..]),
    ] {
        let (output, directory) = invoke(&["help", "language", topic]);
        assert!(output.status.success() && output.stderr.is_empty());
        let source = String::from_utf8(output.stdout).unwrap();
        assert!(source.len() <= 5_000);
        for slug in slugs {
            assert!(source.contains(&format!("help language detail:{slug}")));
        }
        std::fs::remove_dir(directory).unwrap();
    }
    let (output, directory) = invoke(&["help", "language", "all"]);
    assert!(output.status.success() && output.stderr.is_empty());
    let reference = std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("docs/AGENT-QUICK-REFERENCE.md"),
    )
    .unwrap();
    assert_eq!(output.stdout, reference);
    for (_, expected) in ORIGINAL {
        assert!(std::str::from_utf8(&output.stdout)
            .unwrap()
            .contains(expected));
    }
    std::fs::remove_dir(directory).unwrap();
}
