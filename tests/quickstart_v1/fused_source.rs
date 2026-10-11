//! Real explicit profile dispatch; the immutable original fixture is unchanged.
use super::*;
const SOURCE: &str = include_str!("../fixtures/record-traversal-maximum.spx");
const ACCOUNT: &str = "16777216"; // Explicit caller choice, never a production default.
fn packet(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid profile packet: {error}; {:?}; {:?}",
            output.stdout, output.stderr
        )
    })
}
#[test]
fn explicit_fused_source_run_build_original_4096_and_account_refusal_keep_default_closed() {
    let fixture = Fixture::new("fused-source");
    std::fs::write(fixture.root.join("input.spx"), SOURCE).unwrap();
    let default = cli(&fixture.root, &["run", "input.spx", "--json"]);
    assert!(!default.status.success());
    assert!(
        String::from_utf8_lossy(&default.stdout).contains("SPX-T267")
            || String::from_utf8_lossy(&default.stderr).contains("SPX-T267")
    );
    for _ in 0..4 {
        let output = cli(
            &fixture.root,
            &[
                "run",
                "input.spx",
                "--profile",
                "fused-construction-v1",
                "--managed-bytes",
                ACCOUNT,
                "--json",
            ],
        );
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let value = packet(&output);
        assert_eq!(value["result"], 8_386_560);
        assert_eq!(value["target"], "interpreter");
        assert_eq!(value["schema"], "semaprax.fused-source.v1");
        assert_eq!(value["max_steps"], 1_000_000);
        let used = value["managed_reserved"].as_u64().unwrap();
        for bytes in [used, used - 1] {
            let output = cli(
                &fixture.root,
                &[
                    "run",
                    "input.spx",
                    "--profile",
                    "fused-construction-v1",
                    "--managed-bytes",
                    &bytes.to_string(),
                    "--json",
                ],
            );
            let actual = packet(&output);
            assert_eq!(output.status.success(), bytes == used);
            if bytes == used {
                assert_eq!(actual["result"], 8_386_560);
            } else {
                assert_eq!(actual["outcome"], "administrative_refusal");
                assert!(actual.get("result").is_none());
            }
        }
    }
    let native_run = cli(
        &fixture.root,
        &[
            "run",
            "input.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--native",
            "--json",
        ],
    );
    assert!(
        native_run.status.success(),
        "{}",
        String::from_utf8_lossy(&native_run.stderr)
    );
    let native_packet = packet(&native_run);
    assert_eq!(native_packet["result"], 8_386_560);
    assert_eq!(native_packet["target"], "native");
    assert_eq!(native_packet["schema"], "semaprax.fused-source.v1");
    let output = cli(
        &fixture.root,
        &[
            "build",
            "input.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--target",
            "native",
            "--output",
            "native",
            "--json",
        ],
    );
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    for _ in 0..4 {
        let output = Command::new(fixture.root.join("native")).output().unwrap();
        assert!(output.status.success());
        assert_eq!(packet(&output)["result"], 8_386_560);
    }
    let before = std::fs::read(fixture.root.join("native")).unwrap();
    let repeat = cli(
        &fixture.root,
        &[
            "build",
            "input.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--target",
            "native",
            "--output",
            "native",
        ],
    );
    assert!(!repeat.status.success());
    assert_eq!(std::fs::read(fixture.root.join("native")).unwrap(), before);
    std::fs::write(
        fixture.root.join("ordinary.spx"),
        "module ordinary; @id(\"ordinary.main\") fn main() -> i64 { 1 }",
    )
    .unwrap();
    let refused = cli(
        &fixture.root,
        &[
            "build",
            "ordinary.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--target",
            "native",
            "--output",
            "refused-artifact",
            "--json",
        ],
    );
    assert!(!refused.status.success());
    assert!(!fixture.root.join("refused-artifact").exists());
    assert_eq!(
        std::fs::read_to_string(fixture.root.join("input.spx")).unwrap(),
        SOURCE
    );
}
#[test]
fn fused_source_options_deny_wrong_profile_project_fuel_and_unrelated_routes_before_io() {
    let fixture = Fixture::new("fused-options");
    for args in [
        vec![
            "run",
            "missing.spx",
            "--profile",
            "wrong",
            "--managed-bytes",
            ACCOUNT,
        ],
        vec!["run", "missing.spx", "--profile", "fused-construction-v1"],
        vec!["run", "missing.spx", "--managed-bytes", ACCOUNT],
        vec![
            "run",
            "missing.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            "0",
        ],
        vec![
            "run",
            "missing.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            "01",
        ],
        vec![
            "run",
            "missing.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            "1073741825",
        ],
        vec![
            "run",
            "missing.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--native",
            "--max-steps",
            "100",
        ],
        vec![
            "run",
            "--manifest-path",
            "missing.toml",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
        ],
        vec![
            "test",
            "missing.toml",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
        ],
        vec![
            "build",
            "missing.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--target",
            "web",
            "--output",
            "refused",
        ],
        vec![
            "build",
            "missing.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--target",
            "native",
            "--export",
            "app.main",
            "--output",
            "refused",
        ],
        vec![
            "build",
            "--manifest-path",
            "missing.toml",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--target",
            "native",
            "--output",
            "refused",
        ],
    ] {
        let output = cli(&fixture.root, &args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(output.stdout.is_empty());
        assert!(!fixture.root.join("refused").exists());
    }
    let build = build_cli::parse(
        &[
            "input.spx",
            "--profile",
            "fused-construction-v1",
            "--managed-bytes",
            ACCOUNT,
            "--target",
            "wasm",
        ]
        .map(str::to_owned),
    )
    .unwrap();
    assert_eq!(build.fused_account.unwrap().bytes(), 16 * 1024 * 1024);
    assert!(matches!(build.input, build_cli::BuildInput::Source(_)));
}
