//! Frozen pairwise oracle versus sorted, separately owned validation IDs.
use super::super::public_example::{compile_native, execute};
use super::{REQUEST_SCHEMA, SCHEDULE};
use semaprax::{codegen, format, graph, interpreter};

const ORIGINAL: &str = include_str!("shiftsim_id_validation/original_helper.txt");
fn section<'a>(source: &'a str, first: &str, next: &str) -> &'a str {
    let begin = source.find(first).unwrap();
    let end = source[begin..].find(next).unwrap() + begin;
    &source[begin..end]
}
fn source(count: usize, identifiers: &str, expected: bool) -> String {
    let patient = section(
        REQUEST_SCHEMA,
        "@id(\"shiftsim.patient\")",
        "@id(\"shiftsim.request\")",
    );
    let sorted = section(
        SCHEDULE,
        "@id(\"shiftsim.schedule.unique_identifiers\")",
        "@id(\"shiftsim.schedule.unique_patient_ids\")",
    );
    let adopted = section(
        SCHEDULE,
        "@id(\"shiftsim.schedule.unique_patient_ids\")",
        "@id(\"shiftsim.schedule.patient_fields_invalid\")",
    );
    let old = ORIGINAL
        .replace(
            "shiftsim.schedule.unique_patient_ids",
            "shiftsim.oracle.unique_patient_ids",
        )
        .replace("fn unique_patient_ids(", "fn original_unique_patient_ids(");
    format!(
        r#"module shiftsim.ids;
{patient}
{sorted}
{adopted}
{old}
@id("app.main") fn main()->i64 {{
 let mut people=vec_with_capacity<Patient>({count}usize);
 let mut index=0usize;
 while index<{count}usize {{
  let identifier={identifiers};
  people=vec_push<Patient>(people,Patient{{priority:1,arrival:0,id:identifier,service:1,deadline:1}});
  index=index+1usize;
  0
 }}
 let before=original_unique_patient_ids(people);
 let after=unique_patient_ids(people);
 if before=={expected} && after=={expected} {{0}}else{{1}}
}}
"#
    )
}
#[test]
fn sorted_patient_id_validation_matches_frozen_pairwise_bytes_and_256_bound() {
    assert!(ORIGINAL.contains("previous_index < index"));
    assert!(!SCHEDULE.contains("previous_index < index"));
    let root = crate::standard_library::temporary("shiftsim-sorted-id-control");
    std::fs::create_dir_all(&root).unwrap();
    for (name, count, ids, expected) in [
        ("empty", 0, "string_format(\"P{}\",index)", true),
        ("one", 1, "\"P0\"", true),
        ("duplicate", 2, "\"P0\"", false),
        (
            "unicode-nul",
            2,
            "if index==0usize {\"é\\0\"}else{\"é\"}",
            true,
        ),
        ("unicode-nul-duplicate", 2, "\"é\\0\"", false),
        ("max-distinct", 256, "string_format(\"P{}\",index)", true),
        (
            "max-distant-duplicate",
            256,
            "if index==255usize {\"P0\"}else{string_format(\"P{}\",index)}",
            false,
        ),
    ] {
        let source = source(count, ids, expected);
        let path = root.join("app.spx");
        let ast = semaprax::check(&source, &path).unwrap_or_else(|d| panic!("{name}: {d:?}"));
        let canonical = format::canonical(&ast);
        let reparsed = semaprax::check(&canonical, &path).unwrap();
        assert_eq!(format::canonical(&reparsed), canonical);
        let projection = graph::to_json(&reparsed).unwrap();
        graph::verify_json(&reparsed, &projection).unwrap();
        std::fs::write(&path, &canonical).unwrap();
        for _ in 0..2 {
            let result =
                interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
            assert!(result.returned, "{name}: {}", result.envelope);
            interpreter::verify_envelope(&result.envelope).unwrap();
            let wire: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            assert_eq!(wire["payload"]["outcome"]["value"], "0", "{name}");
        }
        let c = codegen::emit_c(&reparsed).unwrap();
        for optimization in ["-O0", "-O2"] {
            let executable = compile_native(&c, &root, optimization);
            for _ in 0..2 {
                let output = execute(&executable, &[]);
                assert!(output.status.success(), "{name} {optimization}: {output:?}");
                assert_eq!(output.stdout, b"0\n", "{name}");
                assert!(output.stderr.is_empty(), "{name}: {output:?}");
            }
        }
    }
    std::fs::remove_dir_all(root).unwrap();
}
