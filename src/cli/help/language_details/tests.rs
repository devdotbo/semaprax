use super::*;
const ORIGINAL: &[(&str, &str)] = &[
    (
        "records",
        include_str!("../../../../tests/cli_help_surface_v1/language_details/records.md"),
    ),
    (
        "io",
        include_str!("../../../../tests/cli_help_surface_v1/language_details/io.md"),
    ),
    (
        "boxes",
        include_str!("../../../../tests/cli_help_surface_v1/language_details/boxes.md"),
    ),
    (
        "record-traversal",
        include_str!("../../../../tests/cli_help_surface_v1/language_details/record-traversal.md"),
    ),
    (
        "field-views",
        include_str!("../../../../tests/cli_help_surface_v1/language_details/field-views.md"),
    ),
    (
        "imports",
        include_str!("../../../../tests/cli_help_surface_v1/language_details/imports.md"),
    ),
];
#[test]
fn all_six_exact_details_preserve_complete_original_heading_and_paragraph_bytes() {
    assert_eq!(DETAILS.len(), 6);
    for (slug, expected) in ORIGINAL {
        let query = format!("detail:{slug}");
        let actual = super::super::language_topic(&query).unwrap();
        assert_eq!(actual, *expected, "{query}");
        assert!(actual.len() <= MAX_BYTES);
        assert!(actual.starts_with("### ") && actual.ends_with('\n'));
        assert!(!actual.contains("\n### ") && !actual.contains("\n## "));
        assert_eq!(
            lookup(&query, super::super::LANGUAGE_REFERENCE).unwrap(),
            *expected
        );
    }
}
#[test]
fn exact_details_refuse_unknown_aliases_and_source_heading_drift() {
    let reference = super::super::LANGUAGE_REFERENCE;
    for query in [
        "detail:",
        "detail:Records",
        "detail:record",
        "detail:records:0",
        "detail:records ",
        "detail:all",
        "detail:topics",
        "detail:record-profile-details",
        "detail:récords",
    ] {
        assert_eq!(
            super::super::language_topic(query).unwrap_err(),
            format!("language card has no exact detail `{query}`")
        );
    }
    assert!(lookup("detail:records", "").is_err());
    assert!(lookup(
        "detail:records",
        "### Detailed profile rules\n\n### Record profile details\n\nwrong level\n"
    )
    .is_err());
    let wrong_level = reference.replace(
        "### Record profile details\n",
        "#### Record profile details\n",
    );
    assert!(lookup("detail:records", &wrong_level).is_err());
    assert!(lookup(
        "detail:records",
        &format!("{reference}\n## Detailed profile rules\n")
    )
    .is_err());
    let missing = reference.replace(
        "### Record profile details\n",
        "### Changed record heading\n",
    );
    assert!(lookup("detail:records", &missing).is_err());
    let duplicate = reference.replace(
        "### Record profile details\n",
        "### Record profile details\n\nfirst\n\n### Record profile details\n",
    );
    assert!(lookup("detail:records", &duplicate).is_err());
    let empty =
        "## Detailed profile rules\n\n### Record profile details\n\n## Where the rules live\n";
    assert!(lookup("detail:records", empty).is_err());
    let outside = format!("### Record profile details\n\noutside appendix\n\n{reference}");
    assert_eq!(lookup("detail:records", &outside).unwrap(), ORIGINAL[0].1);
}
#[test]
fn detail_bounds_fail_closed_after_full_link_expansion_without_truncation() {
    let heading = "### Record profile details\n\n";
    for size in [MAX_BYTES, MAX_BYTES + 1] {
        let section = format!("{heading}{}\n", "x".repeat(size - heading.len() - 1));
        let reference =
            format!("## Detailed profile rules\n\n{section}\n## Where the rules live\n");
        match lookup("detail:records", &reference) {
            Ok(actual) => {
                assert_eq!(size, MAX_BYTES);
                assert_eq!(actual, section);
            }
            Err(error) => {
                assert_eq!(size, MAX_BYTES + 1);
                assert_eq!(error, "language detail `detail:records` exceeds 2048 bytes");
            }
        }
    }
    let reference = format!("## Detailed profile rules\n\n{heading}[complete][rule]\n\n## Where the rules live\n[rule]: {}\n", "x".repeat(MAX_BYTES));
    assert_eq!(
        lookup("detail:records", &reference).unwrap_err(),
        "language detail `detail:records` exceeds 2048 bytes"
    );
}
