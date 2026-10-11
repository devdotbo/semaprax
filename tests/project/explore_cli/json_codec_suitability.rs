//! Actual public read-only CLI: exact checked source receipts, bounded whole-row
//! continuation and capability/shape separation. No generated source is installed.
use super::*;
use std::collections::BTreeMap;

const FILES: [&str; 4] = [
    "semaprax.toml",
    "src/app.spx",
    "src/core.spx",
    "src/tests.spx",
];
const TYPE: &str = "explain.Row";
const MANIFEST: &str = r#"schema = "semaprax.manifest.v1"

[package]
name = "calculator"
version = "0.1.0"

[modules]
entry = "calculator.app"
sources = ["src/app.spx", "src/core.spx", "src/tests.spx"]
tests = ["calculator.tests"]

[exports]
web = ["calculator.add", "calculator.divide", "calculator.is-negative", "calculator.multiply", "calculator.not", "calculator.subtract"]

[targets]
matrix = ["native64"]
"#;

struct CheckedFixture(Fixture);
impl Drop for CheckedFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0 .0);
    }
}
impl CheckedFixture {
    fn new() -> Self {
        let fixture = Fixture::new();
        std::fs::write(fixture.0.join("semaprax.toml"), MANIFEST).unwrap();
        let core = std::fs::read_to_string(fixture.0.join("src/core.spx")).unwrap();
        let functions = core.strip_prefix("module calculator.core;\n").unwrap();
        std::fs::write(fixture.0.join("src/core.spx"), format!(
            "module calculator.core;\n\n@id(\"explain.Row\")\nrecord Row {{\n    @id(\"explain.Row.value\")\n    value: i64,\n}}\n{functions}"
        )).unwrap();
        let result = Self(fixture);
        // Establish the ordinary authenticated read boundary before checking
        // that the CLI publishes nothing.
        result.bindings();
        result
    }
    fn bindings(&self) -> (String, String) {
        with_authenticated_project(&self.0 .0.join("semaprax.toml"), |snapshot| {
            let revision = snapshot.retain_revision();
            Ok((
                revision.project_revision().to_owned(),
                revision.workspace_revision().to_owned(),
            ))
        })
        .unwrap()
    }
    fn explain(&self, options: &[&str]) -> Output {
        let before = tree(&self.0 .0);
        let mut args = vec!["json-codec", "semaprax.toml", "--explain", "--type", TYPE];
        args.extend_from_slice(options);
        let output = self.0.cli(&args);
        assert_eq!(
            tree(&self.0 .0),
            before,
            "explain changed files or published an artifact"
        );
        output
    }
}
fn tree(root: &Path) -> BTreeMap<PathBuf, Option<Vec<u8>>> {
    fn walk(root: &Path, path: &Path, result: &mut BTreeMap<PathBuf, Option<Vec<u8>>>) {
        for entry in std::fs::read_dir(path).unwrap() {
            let entry = entry.unwrap();
            let kind = entry.file_type().unwrap();
            let path = entry.path();
            let relative = path.strip_prefix(root).unwrap().to_owned();
            if kind.is_dir() {
                result.insert(relative, None);
                walk(root, &path, result);
            } else {
                assert!(kind.is_file());
                result.insert(relative, Some(std::fs::read(path).unwrap()));
            }
        }
    }
    let mut result = BTreeMap::new();
    walk(root, root, &mut result);
    result
}
fn success(output: &Output, maximum: usize) -> Value {
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert!(output.stdout.len() <= maximum);
    assert!(output.stdout.ends_with(b"\n"));
    serde_json::from_slice(&output.stdout).unwrap()
}
fn refusal(output: &Output, code: Option<&str>) {
    assert!(!output.status.success());
    assert!(output.stdout.is_empty());
    if let Some(code) = code {
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(code),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
    } else {
        assert_eq!(output.status.code(), Some(2));
        assert!(String::from_utf8_lossy(&output.stderr).contains("json-codec <project> --explain"));
    }
}
fn receipt(page: &Value, bindings: &(String, String), source_hash: &str) {
    assert_eq!(page["schema"], "semaprax.json-codec-suitability.v1");
    assert_eq!(page["project_revision"], bindings.0);
    assert_eq!(page["workspace_revision"], bindings.1);
    assert_eq!(page["source"], "src/core.spx");
    assert_eq!(page["source_sha256"], source_hash);
    assert_eq!(page["type"], TYPE);
    assert_eq!(page["profile_count"], 12);
    for row in page["profiles"].as_array().unwrap() {
        assert_eq!(row["candidate_admission"], "not_evaluated");
        assert_eq!(row["backend_admission"], "not_evaluated");
        assert!(
            row.get("source").is_none(),
            "a generated replacement is not explanation output"
        );
    }
}
fn source_hash(fixture: &CheckedFixture) -> String {
    let bytes = std::fs::read(fixture.0 .0.join("src/core.spx")).unwrap();
    let digest = Sha256::digest(bytes);
    format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    )
}

#[test]
fn public_explain_defaults_and_exact_pages_bind_all_profiles_without_publication() {
    let fixture = CheckedFixture::new();
    let binding = fixture.bindings();
    let digest = source_hash(&fixture);
    let first = fixture.explain(&[]);
    let first_value = success(&first, 4096);
    receipt(&first_value, &binding, &digest);
    assert_eq!(first_value["target"], "native64");
    assert_eq!(
        first_value["policy"],
        json!({"max_string_bytes":null,"max_array_items":null})
    );
    assert!(first_value["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| !row["missing_policy"].as_array().unwrap().is_empty()));
    assert_eq!(fixture.explain(&[]).stdout, first.stdout);
    let options = [
        "--target",
        "native64",
        "--max-string-bytes",
        "64",
        "--max-array-items",
        "256",
        "--max-bytes",
        "16384",
    ];
    let full = fixture.explain(&options);
    let value = success(&full, 16384);
    receipt(&value, &binding, &digest);
    assert_eq!(value["coverage"], "complete");
    assert!(value["next_offset"].is_null());
    let rows = value["profiles"].as_array().unwrap();
    assert_eq!(rows.len(), 12);
    assert_eq!(rows[0]["selector"], "flat-scalars.v1");
    assert_eq!(rows[0]["shape_matches"], true);
    assert!(!rows[0]["required_modules"].as_array().unwrap().is_empty());
    assert!(!rows[0]["imports_absent_from_current_entry_index"]
        .as_array()
        .unwrap()
        .is_empty());
    assert!(!rows[0]["remaining_prerequisites"]
        .as_array()
        .unwrap()
        .is_empty());
    assert_eq!(
        rows.iter()
            .map(|row| row["selector"].as_str().unwrap())
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        12
    );
    let exact = full.stdout.len().to_string();
    let plus = (full.stdout.len() + 1).to_string();
    let short = (full.stdout.len() - 1).to_string();
    assert!(full.stdout.len() > 512 && full.stdout.len() < 16384);
    for bound in [&exact, &plus] {
        let mut args = options;
        args[7] = bound;
        assert_eq!(fixture.explain(&args).stdout, full.stdout);
    }
    let mut args = options;
    args[7] = &short;
    let short_page = success(&fixture.explain(&args), full.stdout.len() - 1);
    assert_eq!(short_page["coverage"], "continued");
    assert!(short_page["next_offset"].is_u64());
    let mut offset = 0usize;
    let mut collected = Vec::new();
    loop {
        let index = offset.to_string();
        let page = success(
            &fixture.explain(&[
                "--target",
                "native64",
                "--max-string-bytes",
                "64",
                "--max-array-items",
                "256",
                "--max-bytes",
                "2048",
                "--revision",
                &binding.0,
                "--offset",
                &index,
            ]),
            2048,
        );
        receipt(&page, &binding, &digest);
        assert_eq!(page["offset"], offset);
        collected.extend(page["profiles"].as_array().unwrap().iter().cloned());
        if let Some(next) = page["next_offset"].as_u64() {
            assert!(next as usize > offset);
            offset = next as usize;
            assert!(offset < 12);
        } else {
            assert_eq!(page["coverage"], "complete");
            break;
        }
    }
    assert_eq!(&collected, rows);
    assert_eq!(
        fixture.explain(&["--source", "src/core.spx"]).stdout,
        first.stdout
    );
}

#[test]
fn public_explain_distinguishes_target_policy_and_refuses_stale_source_receipts() {
    let fixture = CheckedFixture::new();
    let old = fixture.bindings();
    let native = success(
        &fixture.explain(&["--target", "native64", "--max-bytes", "16384"]),
        16384,
    );
    let wasm = success(
        &fixture.explain(&["--target", "wasm32", "--max-bytes", "16384"]),
        16384,
    );
    assert_eq!(native["target_manifest_policy"]["manifest_allows"], true);
    assert_eq!(wasm["target_manifest_policy"]["manifest_allows"], false);
    assert_eq!(
        native["profiles"], wasm["profiles"],
        "target policy cannot alter source shape"
    );
    let original = FILES.map(|p| std::fs::read(fixture.0 .0.join(p)).unwrap());
    let core = String::from_utf8(original[2].clone())
        .unwrap()
        .replace("value: i64,", "value: usize,");
    std::fs::write(fixture.0 .0.join("src/core.spx"), core).unwrap();
    refusal(
        &fixture.explain(&["--revision", &old.0, "--offset", "1"]),
        Some("SPX-J180"),
    );
    let current = fixture.bindings();
    assert_ne!(current.0, old.0);
    let fresh = success(&fixture.explain(&[]), 4096);
    receipt(&fresh, &current, &source_hash(&fixture));
    assert_ne!(fresh["source_sha256"], native["source_sha256"]);
    let before_unknown = tree(&fixture.0 .0);
    let unknown = fixture.0.cli(&[
        "json-codec",
        "semaprax.toml",
        "--explain",
        "--type",
        "absent",
    ]);
    refusal(&unknown, Some("SPX-J180"));
    assert_eq!(tree(&fixture.0 .0), before_unknown);
    refusal(
        &fixture.explain(&["--source", "src/app.spx"]),
        Some("SPX-J180"),
    );
    // A manifest change also invalidates the receipt, while the exact checked
    // source/type digest stays unchanged. It changes capability, not shape.
    let digest = source_hash(&fixture);
    let source_profiles = success(&fixture.explain(&["--max-bytes", "16384"]), 16384);
    std::fs::write(
        fixture.0 .0.join("semaprax.toml"),
        MANIFEST.replace("matrix = [\"native64\"]", "matrix = [\"wasm32\"]"),
    )
    .unwrap();
    refusal(
        &fixture.explain(&["--revision", &current.0, "--offset", "1"]),
        Some("SPX-J180"),
    );
    let changed_manifest = fixture.bindings();
    assert_ne!(changed_manifest.0, current.0);
    let changed = success(&fixture.explain(&[]), 4096);
    receipt(&changed, &changed_manifest, &digest);
    assert_eq!(changed["target"], "wasm32");
    let manifest_profiles = success(&fixture.explain(&["--max-bytes", "16384"]), 16384);
    assert_eq!(manifest_profiles["profiles"], source_profiles["profiles"]);
}

#[test]
fn public_explain_rejects_generation_flags_duplicate_options_and_unbound_continuations() {
    let fixture = CheckedFixture::new();
    let original = tree(&fixture.0 .0);
    for options in [
        vec!["--output", "generated"],
        vec!["--plan", "plan.json"],
        vec!["--profile", "scalar-record.v1"],
        vec!["--type", "another"],
        vec!["--target", "hosted"],
        vec!["--max-bytes", "0512"],
        vec!["--max-string-bytes", "65"],
        vec!["--max-array-items", "257"],
        vec!["--offset", "1"],
        vec!["--offset", "12"],
    ] {
        refusal(&fixture.explain(&options), None);
    }
    assert_eq!(tree(&fixture.0 .0), original);
}
