//! Actual host refusals must settle before subsequent declared test roots.
use super::*;
use serde_json::Value;

fn cases(output: &Output, passed: bool) -> Vec<Value> {
    assert_eq!(
        output.status.code(),
        Some(if passed { 0 } else { 1 }),
        "{}",
        stderr(output)
    );
    let envelope = json(output);
    assert_eq!(envelope["schema"], "semaprax.native-test.v1");
    assert_eq!(envelope["passed"], passed);
    let cases = envelope["cases"].as_array().unwrap().clone();
    assert_eq!(cases.len(), 3);
    assert_eq!(
        cases
            .iter()
            .map(|case| case["stable_id"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "decimal.tests.main",
            "decimal.tests.test_a_refusal",
            "decimal.tests.test_z_after"
        ]
    );
    assert_eq!(cases[0]["passed"], true, "main control: {}", envelope);
    assert_eq!(cases[2]["passed"], true, "following control: {}", envelope);
    assert_eq!(cases[2]["result"], 0);
    assert_eq!(cases[2]["exit_code"], 0);
    cases
}

fn settle(fixture: &NativeFixture, source_before: &[u8]) {
    assert_eq!(
        std::fs::read(fixture.root.join("tests.spx")).unwrap(),
        source_before
    );
    fixture.scratch_empty();
    assert_eq!(std::fs::read_dir(&fixture.scratch).unwrap().count(), 0);
}

fn repair(fixture: &NativeFixture, source: &str) {
    let path = fixture.root.join("tests.spx");
    let parsed = semaprax::parse(source, &path).unwrap();
    std::fs::write(path, semaprax::format::canonical(&parsed)).unwrap();
}

#[test]
#[cfg_attr(windows, ignore = "native SourceCommand adapter is Unix-only")]
fn timeout_settles_before_next_case_and_authenticated_reentry() {
    let source = r#"module decimal.tests;
@id("decimal.tests.main") fn main()->i64 {0}
@id("decimal.tests.test_a_refusal") fn test_a_refusal()->i64 {while true {0} 0}
@id("decimal.tests.test_z_after") fn test_z_after()->i64 {0}
"#;
    let fixture = NativeFixture::new("timeout-recovery", source);
    let original = std::fs::read(fixture.root.join("tests.spx")).unwrap();
    // Preserve the exact deadline and acceptance controls. Opt-in observations
    // distinguish per-case setup, deadline and settlement on an actual refusal;
    // these timestamps never decide whether a case passes.
    let arguments = [
        "test",
        "--target",
        "native",
        "--native-timeout-ms",
        "100",
        "--native-timing",
        "--json",
    ];
    let failed = fixture.cli(&arguments);
    let rows = cases(&failed, false);
    assert_eq!(rows[1]["passed"], false);
    assert!(rows[1]["result"].is_null());
    assert!(rows[1]["outcome"]
        .as_str()
        .unwrap()
        .contains("exceeded 100 ms"));
    assert_eq!(rows[1]["stdout"], "");
    assert_eq!(rows[2]["stdout"], "0\n");
    assert_eq!(rows[2]["stderr"], "");
    settle(&fixture, &original);

    // A new canonical source revision is independently authenticated. The
    // former failed root and the same scratch parent must not poison reentry.
    repair(&fixture, &source.replace("while true {0} 0", "0"));
    let repaired = std::fs::read(fixture.root.join("tests.spx")).unwrap();
    let accepted = fixture.cli(&arguments);
    assert!(cases(&accepted, true)
        .iter()
        .all(|case| case["passed"] == true));
    settle(&fixture, &repaired);
}

#[test]
#[cfg_attr(windows, ignore = "native SourceCommand adapter is Unix-only")]
fn combined_output_refusal_settles_before_next_case_and_authenticated_reentry() {
    let source = r#"module decimal.tests;
permit {process.stdout.write,process.stderr.write}
@id("decimal.tests.main")
fn main()->i64 uses {process.stdout.write,process.stderr.write} {
 let bytes=[109u8]; let out=stdout_write(array_as_slice(bytes));
 let err=stderr_write(array_as_slice(bytes)); 0
}
@id("decimal.tests.test_a_refusal")
fn test_a_refusal()->i64 uses {process.stdout.write,process.stderr.write} {
 let bytes=[97u8,98u8,99u8,100u8,101u8,102u8,103u8,104u8];
 let out=stdout_write(array_as_slice(bytes)); let err=stderr_write(array_as_slice(bytes)); 0
}
@id("decimal.tests.test_z_after")
fn test_z_after()->i64 uses {process.stdout.write,process.stderr.write} {
 let bytes=[122u8]; let out=stdout_write(array_as_slice(bytes));
 let err=stderr_write(array_as_slice(bytes)); 0
}
"#;
    let fixture = NativeFixture::new("output-recovery", source);
    let original = std::fs::read(fixture.root.join("tests.spx")).unwrap();
    let arguments = [
        "test",
        "--target",
        "native",
        "--native-max-output-bytes",
        "12",
        "--json",
    ];
    let failed = fixture.cli(&arguments);
    let rows = cases(&failed, false);
    assert_eq!(rows[1]["passed"], false);
    assert!(rows[1]["result"].is_null());
    assert!(rows[1]["outcome"]
        .as_str()
        .unwrap()
        .contains("exceeded 12 output bytes"));
    assert_eq!(rows[0]["stdout"], "m");
    assert_eq!(rows[0]["stderr"], "m");
    assert_eq!(rows[2]["stdout"], "z");
    assert_eq!(rows[2]["stderr"], "z");
    settle(&fixture, &original);

    repair(
        &fixture,
        &source.replace("[97u8,98u8,99u8,100u8,101u8,102u8,103u8,104u8]", "[114u8]"),
    );
    let repaired = std::fs::read(fixture.root.join("tests.spx")).unwrap();
    let accepted = fixture.cli(&arguments);
    let rows = cases(&accepted, true);
    assert!(rows.iter().all(|case| case["passed"] == true));
    assert_eq!(rows[1]["stdout"], "r");
    assert_eq!(rows[1]["stderr"], "r");
    settle(&fixture, &repaired);
}
