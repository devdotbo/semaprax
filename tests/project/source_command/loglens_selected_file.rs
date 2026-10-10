//! Real LogLens source, native O0/O2 and every unchanged CLI49 acceptance body.
use super::{project, Command, Fixture};
use std::path::Path;

const APP: &str = include_str!("../../../experiments/selected-file-input/loglens/loglens.spx");
const UNIT: &str = include_str!("../../../experiments/selected-file-input/loglens/unit.spx");
const MANIFEST: &str =
    include_str!("../../../experiments/selected-file-input/loglens/semaprax.toml");
const RUN: &str = include_str!("../../../experiments/selected-file-input/loglens/run.sh");
const TESTS: &str = include_str!("../../../experiments/selected-file-input/loglens/tests.mjs");

fn fixture() -> Fixture {
    Fixture::with_tests(
        APP,
        &MANIFEST
            .replace("loglens.spx", "app.spx")
            .replace("unit.spx", "tests.spx"),
        UNIT,
    )
}

fn executable(path: &Path, bytes: &str) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::write(path, bytes).unwrap();
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

fn source(fixture: &Fixture) -> String {
    project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
        semaprax::codegen::emit_hir_c_with_selected_file_resource_command(snapshot.entry_program())
            .map_err(|error| vec![error])
    })
    .unwrap()
}

#[test]
fn original_cli49_and_all_retained_assertions_run_on_native_o0_o2() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    for optimization in ["-O0", "-O2"] {
        let fixture = fixture();
        std::fs::write(fixture.0.join("selected.c"), source(&fixture)).unwrap();
        executable(&fixture.0.join("run.sh"), RUN);
        std::fs::write(fixture.0.join("tests.mjs"), TESTS).unwrap();
        // This owning build compiles the exact validated current HIR at each
        // optimization. It does not substitute a prior binary or a no-op build.
        executable(&fixture.0.join("build.sh"), &format!(
            "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nclang -std=c11 -Wall -Wextra -Werror {optimization} selected.c -o loglens\n"));
        executable(
            &fixture.0.join("test.sh"),
            "#!/bin/sh\nset -eu\ncd \"$(dirname \"$0\")\"\nnode tests.mjs\n",
        );
        let output = Command::new("python3")
            .arg(repo.join("benchmarks/cli-tokens-v1/selected_file_adoption.py"))
            .arg("--candidate")
            .arg(&fixture.0)
            .arg("--output")
            .arg(fixture.0.join("qualification"))
            .env(
                "LOGLENS_SAMPLE",
                repo.join("benchmarks/cli-tokens-v1/sample.log"),
            )
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{optimization}: {}\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(report["accepted"], true);
        assert_eq!(report["historical_checks"], 33);
        assert_eq!(report["boundary_checks"], 16);
        assert_eq!(report["benchmark_executed"], false);
    }
}

#[test]
fn selected_file_wrapper_has_no_input_path_or_option_adapter() {
    assert_eq!(RUN, "#!/bin/sh\nset -eu\napp_dir=$(CDPATH= cd -- \"$(dirname -- \"$0\")\" && pwd)\nif [ ! -x \"$app_dir/loglens\" ]; then \"$app_dir/build.sh\" >/dev/null; fi\nif [ \"$#\" -eq 0 ]; then exec \"$app_dir/loglens\"; fi\nexec \"$app_dir/loglens\" --spx-selected-file-v1 0 -- \"$@\"\n");
    let original = include_str!(
        "../../../experiments/ascii-pattern-source/loglens-reference/qualified-source/tests.mjs"
    );
    let restored = TESTS
        .replace("const samplePath = process.env.LOGLENS_SAMPLE ?? '../../../benchmarks/cli-tokens-v1/sample.log';\n", "")
        .replace("readFileSync(samplePath, 'utf8')", "readFileSync('../sample.log', 'utf8')")
        .replace("invoke([samplePath", "invoke(['../sample.log'");
    assert_eq!(
        restored, original,
        "all retained assertions must remain verbatim"
    );
}

#[test]
fn selected_paths_and_source_usage_run_on_actual_loglens_o0_o2() {
    let repo = Path::new(env!("CARGO_MANIFEST_DIR"));
    let expected = std::fs::read(repo.join("benchmarks/cli-tokens-v1/expected.json")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let fixture = fixture();
        std::fs::write(fixture.0.join("selected.c"), source(&fixture)).unwrap();
        let binary = fixture.0.join("loglens");
        let result = Command::new("clang")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
            .arg(fixture.0.join("selected.c"))
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let input = fixture.0.join(".hidden sample log");
        std::fs::copy(repo.join("benchmarks/cli-tokens-v1/sample.log"), &input).unwrap();
        std::os::unix::fs::symlink(&input, fixture.0.join("selected-link")).unwrap();
        let nested = fixture.0.join("nested");
        std::fs::create_dir(&nested).unwrap();
        for (cwd, path) in [
            (fixture.0.as_path(), input.to_str().unwrap()),
            (fixture.0.as_path(), ".hidden sample log"),
            (fixture.0.as_path(), "selected-link"),
            (nested.as_path(), "../.hidden sample log"),
        ] {
            let output = Command::new(&binary)
                .current_dir(cwd)
                .args([
                    "--spx-selected-file-v1",
                    "0",
                    "--",
                    path,
                    "--top",
                    "3",
                    "--json",
                ])
                .output()
                .unwrap();
            assert_eq!(
                output.status.code(),
                Some(0),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert_eq!(output.stdout, expected);
            assert!(output.stderr.is_empty());
        }
        for args in [
            vec!["--json"],
            vec!["missing", "--top", "+3"],
            vec!["missing", "--top", "1.5"],
            vec!["absent/dir/file", "--wat"],
        ] {
            let output = Command::new(&binary)
                .current_dir(&fixture.0)
                .args(["--spx-selected-file-v1", "0", "--"])
                .args(args)
                .output()
                .unwrap();
            assert_eq!(output.status.code(), Some(2));
            assert!(output.stdout.is_empty());
            assert_eq!(output.stderr, b"usage: loglens <file> [--top N] [--json]\n");
        }
    }
}
