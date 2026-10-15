use std::fmt::Write as _;

use super::{LIBRARY_CATALOG, LIBRARY_INDEX};

const PATTERN_USAGE: &str = r#"
std.pattern usage (Partial; ASCII byte patterns)
Manifest: [dependencies] std.pattern = "^0.1.0"; library [package] profile = "owned-data-api.v1".
use type @id("std.pattern.matcher") from std.pattern as Matcher;
use function @id("std.pattern.make") from std.pattern as make;
use function @id("std.pattern.compile") from std.pattern as compile;
use function @id("std.pattern.full-match") from std.pattern as full_match;
Call local aliases. Pattern source "\\x41" matches byte A; full_match anchors the entire input.
Name independent pattern/input views (string_as_str then str_as_bytes, or array_as_slice).
let mut m: Matcher = compile(make(), pattern_view, 262144usize);
m = full_match(m, input_view, 262144usize);
Keep each returned owner. Check result_valid before status: 0 ready, 1 match, 2 no-match, 3 invalid, 4 resource.
Captures require status 1 and index < capture_count; byte offsets are relative to that input view.
Guide: std/pattern/README.md
"#;

// Reuse the complete source-owned example rather than a drifting fragment.
const NDJSON_USAGE: &str = concat!(
    "\nstd.data.ndjson usage (authored; qualification pending)\n",
    "Manifest: [dependencies] std.data.ndjson = \"^0.1.0\"; [package] profile = \"owned-data-api.v1\".\n",
    "Check global_fault before framing; body <=65536 bytes, <=256 records; line <=512 bytes.\n",
    "Frame cuts describe CURRENT bytes; frame_matches does not authenticate an owner or generation.\n",
    "basic_fault forms the current receiving range; keep the input owner live. Fault categories: 0 OK, 1 limit, 2 UTF-8, 3 empty.\n",
    "JSON grammar and publication remain the caller's responsibility.\n```spx\n",
    include_str!("../../../std/data-ndjson/src/examples.spx"),
    "```\nGuide: docs/BOUNDED-NDJSON-FRAMING-V1.md\n",
);

pub(super) fn library_help(query: Option<&str>) -> Result<String, String> {
    match query {
        None => Ok(library_index()),
        Some("all") => Ok(LIBRARY_CATALOG.to_owned()),
        Some(query) => library_entry(query),
    }
}

pub(super) fn library_index() -> String {
    let catalog: serde_json::Value =
        serde_json::from_str(LIBRARY_INDEX).expect("generated standard-library JSON must parse");
    let modules = catalog["modules"]
        .as_array()
        .expect("generated standard-library JSON must contain modules");
    let mut output = String::new();
    writeln!(output, "Standard library modules ({}):", modules.len())
        .expect("writing to a string cannot fail");
    for module in modules {
        let id = module["module"]
            .as_str()
            .expect("generated standard-library module must have an identity");
        writeln!(output, "  {id}").expect("writing to a string cannot fail");
    }
    writeln!(
        output,
        "\nExact lookup: semaprax help library <module|name|stable-id>"
    )
    .expect("writing to a string cannot fail");
    writeln!(output, "Full catalog: semaprax help library all")
        .expect("writing to a string cannot fail");
    output
}

pub(super) fn library_entry(query: &str) -> Result<String, String> {
    let catalog: serde_json::Value =
        serde_json::from_str(LIBRARY_INDEX).expect("generated standard-library JSON must parse");
    let modules = catalog["modules"]
        .as_array()
        .expect("generated standard-library JSON must contain modules");
    let mut output = String::new();
    let mut pattern_selected = false;
    let mut ndjson_selected = false;
    for module in modules {
        let module_id = module["module"]
            .as_str()
            .expect("generated standard-library module must have an identity");
        let whole_module = query == module_id;
        for declaration in module["declarations"]
            .as_array()
            .expect("generated standard-library module must contain declarations")
        {
            let id = declaration["id"]
                .as_str()
                .expect("generated standard-library declaration must have an identity");
            let name = declaration["name"]
                .as_str()
                .expect("generated standard-library declaration must have a name");
            if !whole_module && query != id && query != name {
                continue;
            }
            pattern_selected |= module_id == "std.pattern";
            ndjson_selected |= module_id == "std.data.ndjson";
            if !output.is_empty() {
                output.push('\n');
            }
            writeln!(output, "{id}").expect("writing to a string cannot fail");
            writeln!(
                output,
                "dependency {}",
                module["dependency"]
                    .as_str()
                    .expect("generated standard-library dependency must be text")
            )
            .expect("writing to a string cannot fail");
            writeln!(
                output,
                "profile {}",
                module["required_profile"]
                    .as_str()
                    .expect("generated standard-library profile must be text")
            )
            .expect("writing to a string cannot fail");
            for line in declaration["head"]
                .as_array()
                .expect("generated standard-library declaration must have a head")
            {
                writeln!(
                    output,
                    "{}",
                    line.as_str()
                        .expect("generated standard-library head line must be text")
                )
                .expect("writing to a string cannot fail");
            }
        }
    }
    if output.is_empty() {
        Err(format!("standard library has no exact match for `{query}`"))
    } else {
        if pattern_selected {
            output.push_str(PATTERN_USAGE);
        }
        if ndjson_selected {
            output.push_str(NDJSON_USAGE);
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pattern_exact_lookup_adds_one_bounded_usage_card_with_real_imports_and_owner_flow() {
        assert!(PATTERN_USAGE.len() <= 1_024);
        assert!(semaprax::agent_economics::lexical_tokens(PATTERN_USAGE) <= 256);
        for query in ["std.pattern", "std.pattern.compile", "compile"] {
            let entry = library_entry(query).unwrap();
            assert!(entry.ends_with(PATTERN_USAGE), "{query}");
            assert_eq!(entry.matches("std.pattern usage (Partial;").count(), 1);
            for required in [
                "use type @id(\"std.pattern.matcher\") from std.pattern as Matcher;",
                "use function @id(\"std.pattern.compile\") from std.pattern as compile;",
                "use function @id(\"std.pattern.full-match\") from std.pattern as full_match;",
                r#"Pattern source "\\x41" matches byte A"#,
                "m = full_match(m, input_view, 262144usize);",
                "Check result_valid before status",
                "byte offsets are relative to that input view",
            ] {
                assert!(entry.contains(required), "{query}: {required}");
            }
            if query == "std.pattern.compile" {
                assert!(entry.starts_with("std.pattern.compile\ndependency std.pattern"));
                assert!(entry.len() <= 2_048);
                assert!(semaprax::agent_economics::lexical_tokens(&entry) <= 512);
            } else if query == "std.pattern" {
                assert!(entry.len() <= 6_144);
            }
        }
        assert!(!library_entry("std.core.compare")
            .unwrap()
            .contains(PATTERN_USAGE));
        assert!(library_entry("std.pattern.internal.read-token").is_err());
        assert_eq!(library_help(Some("all")).unwrap(), LIBRARY_CATALOG);
    }

    #[test]
    fn ndjson_exact_lookup_keeps_complete_source_example_and_current_receiver_rules() {
        assert!(NDJSON_USAGE.len() <= 1536);
        assert!(semaprax::agent_economics::lexical_tokens(NDJSON_USAGE) <= 384);
        let example = include_str!("../../../std/data-ndjson/src/examples.spx");
        assert!(NDJSON_USAGE.contains(&format!("```spx\n{example}```\n")));
        for query in ["std.data.ndjson", "std.data.ndjson.frame-at", "frame_at"] {
            let entry = library_entry(query).unwrap();
            assert!(entry.ends_with(NDJSON_USAGE), "{query}");
            assert_eq!(entry.matches("std.data.ndjson usage (").count(), 1);
            assert!(entry.contains("CURRENT bytes"));
            assert!(entry.contains("does not authenticate an owner or generation"));
            assert!(entry.contains("Check global_fault before framing"));
            assert!(
                entry.contains("JSON grammar and publication remain the caller's responsibility")
            );
            if query == "std.data.ndjson.frame-at" {
                assert!(entry.len() <= 2048);
                assert!(semaprax::agent_economics::lexical_tokens(&entry) <= 512);
            }
        }
        assert!(!library_entry("std.core.compare")
            .unwrap()
            .contains(NDJSON_USAGE));
        assert!(library_entry("std.data.ndjson.internal.frame").is_err());
        assert_eq!(library_help(Some("all")).unwrap(), LIBRARY_CATALOG);
    }

    #[test]
    fn default_is_a_complete_module_index_and_all_selects_the_full_catalog() {
        let index = library_help(None).unwrap();
        let catalog: serde_json::Value = serde_json::from_str(LIBRARY_INDEX).unwrap();
        let modules = catalog["modules"].as_array().unwrap();
        assert!(index.starts_with(&format!("Standard library modules ({}):\n", modules.len())));
        let indexed: Vec<_> = index
            .lines()
            .filter_map(|line| line.strip_prefix("  "))
            .collect();
        let expected: Vec<_> = modules
            .iter()
            .map(|module| module["module"].as_str().unwrap())
            .collect();
        assert_eq!(indexed, expected);
        assert!(index.contains("\n  std.int.decimal\n"));
        assert!(index.contains("\n  std.data.json.scan\n"));
        assert!(index.contains("\n  std.pattern\n"));
        assert!(index.contains("semaprax help library <module|name|stable-id>"));
        assert!(index.contains("semaprax help library all"));
        assert!(index.len() <= 2_048);
        assert!(semaprax::agent_economics::lexical_tokens(&index) <= 256);
        assert_eq!(
            index
                .lines()
                .filter(|line| line.starts_with("  std."))
                .count(),
            modules.len()
        );
        assert_eq!(library_help(Some("all")).unwrap(), LIBRARY_CATALOG);
    }
}
