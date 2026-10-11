use super::*;
const SOURCE: &str = include_str!("../../tests/fixtures/record-traversal-maximum.spx");
const CALLER_ACCOUNT: usize = 16 * 1024 * 1024;
fn run(source: &str, bytes: usize, fuel: usize) -> (RunReport, serde_json::Value) {
    let report = run_interpreter(
        source,
        Path::new("current.spx"),
        RuntimeAccount::new(bytes).unwrap(),
        fuel,
    )
    .unwrap();
    let value = serde_json::from_str(report.envelope()).unwrap();
    (report, value)
}
#[test]
fn explicit_source_factory_keeps_original_4096_and_ordinary_refusal_with_current_raw_identity() {
    assert!(RuntimeAccount::new(0).is_err());
    assert!(RuntimeAccount::new(MAX_MANAGED_BYTES + 1).is_err());
    assert_eq!(
        RuntimeAccount::new(MAX_MANAGED_BYTES).unwrap().bytes(),
        MAX_MANAGED_BYTES
    );
    let ordinary = crate::check(SOURCE, "current.spx").unwrap_err();
    assert!(ordinary.iter().any(|error| error.code == "SPX-T267"));
    for _ in 0..4 {
        let (report, value) = run(
            SOURCE,
            CALLER_ACCOUNT,
            crate::interpreter::DEFAULT_MAX_STEPS,
        );
        assert!(report.returned());
        assert_eq!(value["result"], 8_386_560);
        assert_eq!(value["profile"], PROFILE);
        assert_eq!(
            value["source_sha256"],
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(SOURCE.as_bytes()))
            )
        );
        assert_eq!(value["max_steps"], crate::interpreter::DEFAULT_MAX_STEPS);
        assert!(
            value["steps_used"].as_u64().unwrap() <= crate::interpreter::DEFAULT_MAX_STEPS as u64
        );
    }
    let non_entry = SOURCE.replace("fn main()", "fn selected_helper()");
    assert!(run_interpreter(
        &non_entry,
        Path::new("non-entry.spx"),
        RuntimeAccount::new(CALLER_ACCOUNT).unwrap(),
        crate::interpreter::DEFAULT_MAX_STEPS,
    )
    .is_err());
    let scalar = "module scalar; @id(\"scalar.main\") fn main() -> i64 { 1 }";
    assert!(crate::check(scalar, "scalar.spx").is_ok());
    assert_eq!(
        run_interpreter(
            scalar,
            Path::new("scalar.spx"),
            RuntimeAccount::new(CALLER_ACCOUNT).unwrap(),
            1000
        )
        .unwrap_err()[0]
            .code,
        "SPX-F130"
    );
}
#[test]
fn interpreter_source_account_exact_short_fuel_and_selected_failure_never_publish_poison() {
    let (_, wide) = run(
        SOURCE,
        CALLER_ACCOUNT,
        crate::interpreter::DEFAULT_MAX_STEPS,
    );
    let exact = wide["managed_reserved"].as_u64().unwrap() as usize;
    assert!(exact > 1);
    let (report, value) = run(SOURCE, exact, crate::interpreter::DEFAULT_MAX_STEPS);
    assert!(report.returned());
    assert_eq!(value["managed_reserved"], exact);
    let (report, value) = run(SOURCE, exact - 1, crate::interpreter::DEFAULT_MAX_STEPS);
    assert!(!report.returned());
    assert_eq!(value["account_refused"], true);
    assert!(value.get("result").is_none());
    let (report, value) = run(SOURCE, CALLER_ACCOUNT, 1);
    assert!(!report.returned());
    assert_eq!(value["outcome"], "fuel_exhausted");
    assert_eq!(value["cleanup_complete"], true);
    assert!(value.get("result").is_none());
    let changed = SOURCE.replace(
        "{ total } else { 0 }",
        "{ total + 9223372036854775807 } else { 0 }",
    );
    assert_ne!(changed, SOURCE);
    let (report, value) = run(
        &changed,
        CALLER_ACCOUNT,
        crate::interpreter::DEFAULT_MAX_STEPS,
    );
    assert!(!report.returned());
    assert_eq!(value["outcome"], "language_failure");
    assert_eq!(value["cleanup_complete"], true);
    assert!(value.get("result").is_none());
    assert_eq!(
        value["normalized_status"],
        serde_json::from_str::<serde_json::Value>(
            &crate::runtime_status::normalize_arithmetic(
                crate::cleanup_plan::StatusCase::AddOverflow
            )
            .to_json()
        )
        .unwrap()
    );
}
#[test]
fn scoped_source_receiving_requested_layout_account_covers_actual_allocations_and_drop() {
    with_selected(SOURCE, Path::new("current.spx"), |owner, _| {
        let ((answer, observed), refused, used) =
            crate::interpreter::construction_fusion::selected::execute_accounted_observed(
                owner,
                crate::interpreter::DEFAULT_MAX_STEPS,
                CALLER_ACCOUNT,
            );
        assert!(!refused);
        assert_eq!(answer.unwrap().result, 8_386_560);
        assert!(!observed.invalid);
        assert_eq!(observed.live, 0);
        assert!(
            observed.allocated <= used,
            "actual runtime frame+Drop allocations must be accounted: {observed:?}, debited={used}"
        );
        Ok(())
    })
    .unwrap();
}
