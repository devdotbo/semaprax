//! Owning `fused-construction-v1` Core-Wasm package definitions (OPT #768).
//! Authored; root executes them after integration. Every package here comes
//! from the unchanged original 4096-row source through `validate_selected`;
//! no rescaled HIR or caller-built grant is involved.
use super::*;
use crate::hir::selected_fused_candidate::{validate_selected, ValidatedFusedSelection};

const ORIGINAL: &str = include_str!("../../../tests/fixtures/record-traversal-maximum.spx");
const EXPECTED: i64 = 8_386_560;

fn source_sha256() -> String {
    sha256_hex(ORIGINAL.as_bytes())
}
fn with_owner<R>(receive: impl FnOnce(&ValidatedFusedSelection<'_>) -> R) -> R {
    let source = crate::parse(ORIGINAL, "fused-source-package.spx").unwrap();
    let owner = validate_selected(&source).expect("original source passes full selected validation");
    receive(&owner)
}
/// One invocation's arena debit, as the paired grant states it.
fn per_invocation(owner: &ValidatedFusedSelection<'_>) -> usize {
    usize::try_from(accept(owner).unwrap().grant.account_bytes).unwrap()
}
/// The eagerly allocated namespace registry the same limit funds first.
fn registry_buffer(owner: &ValidatedFusedSelection<'_>) -> usize {
    usize::try_from(accept(owner).unwrap().grant.registry_entries * REGISTRY_ENTRY_BYTES).unwrap()
}
fn borrowed(files: &[(&'static str, Vec<u8>)]) -> Vec<(&'static str, &[u8])> {
    files.iter().map(|(name, bytes)| (*name, bytes.as_slice())).collect()
}
fn scratch(label: &str) -> std::path::PathBuf {
    let parent = std::env::temp_dir().join(format!(
        "semaprax-fused-source-package-{}-{label}",
        std::process::id()
    ));
    std::fs::create_dir_all(&parent).unwrap();
    parent
}
/// The facade's publication path without the facade type: verify, then the
/// shared atomic writer.
fn publish(files: &[(&'static str, Vec<u8>)], output: &Path) -> Result<(), Diagnostic> {
    let artifacts = borrowed(files);
    verify_files(&artifacts)?;
    super::super::publish_scalar_package(output, &artifacts)
}
fn run_node(directory: &Path, mode: &str) {
    let driver = directory.with_extension("driver.mjs");
    std::fs::write(&driver, include_str!("driver.mjs")).unwrap();
    let output = std::process::Command::new("node")
        .arg(&driver)
        .arg(directory)
        .arg(mode)
        .arg(EXPECTED.to_string())
        .arg(source_sha256())
        .output()
        .expect("Node required by the fused-construction-v1 Core-Wasm package gate");
    std::fs::remove_file(driver).unwrap();
    assert!(
        output.status.success(),
        "fused package {mode} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "fused-source-package-v1-ok"
    );
}

#[test]
fn package_is_deterministic_and_binds_source_strategy_module_and_grant() {
    with_owner(|owner| {
        let funding = registry_buffer(owner) + 3 * per_invocation(owner);
        let first = package_files(owner, funding, &source_sha256()).unwrap();
        assert_eq!(first, package_files(owner, funding, &source_sha256()).unwrap());
        let names: Vec<&str> = first.iter().map(|(name, _)| *name).collect();
        assert_eq!(names, FILES);
        let manifest = std::str::from_utf8(&first[0].1).unwrap();
        let entry = std::str::from_utf8(&first[1].1).unwrap();
        let module = &first[2].1;
        assert_eq!(first[3].1, HOST.as_bytes());
        assert_eq!(module, &accept(owner).unwrap().bytes);
        let digest = sha256_hex(module);
        for bound in [
            format!("\"source_sha256\":\"{}\"", source_sha256()),
            format!("\"module_sha256\":\"{digest}\""),
            format!("\"grant\":{{\"digest\":\"{digest}\",\"rows\":4096,\"width\":3,"),
            format!("\"profile\":\"{PROFILE}\",\"strategy\":\"{STRATEGY}\""),
            "\"target\":\"wasm\"".to_owned(),
            format!("\"result_schema\":\"{RESULT_SCHEMA}\""),
            format!("\"account_bytes\":{funding},\"account_limit_max\":{MAX_ACCOUNT_LIMIT},"),
            format!("\"registry_buffer_bytes\":{},", registry_buffer(owner)),
            format!("\"per_invocation_arena_bytes\":{},", per_invocation(owner)),
            "\"allocation\":\"eager-registry-then-eager-arena-remainder\"".to_owned(),
            "\"bound\":\"funded-buffers-only\"".to_owned(),
            "\"fuel\":\"none\"".to_owned(),
            "\"realm\":\"trusted-first-install\"".to_owned(),
        ] {
            assert!(manifest.contains(&bound), "{bound}");
        }
        assert!(entry.contains(manifest));
        assert!(!entry.contains(MANIFEST_SLOT));
        // The public entry takes only a finite limit and loads only its
        // compiler-paired module through one fixed URL; no module, grant,
        // path or fault input exists.
        assert_eq!(ENTRY_TEMPLATE.matches(MANIFEST_SLOT).count(), 1);
        assert_eq!(ENTRY_TEMPLATE.matches(PAIRED_LOADER).count(), 1);
        assert!(!ENTRY_TEMPLATE.contains("faults"));
        assert!(ENTRY_TEMPLATE.contains("export async function install(accountBytes)"));
        assert!(!ENTRY_TEMPLATE.contains("moduleBytes"));
        assert!(ENTRY_TEMPLATE.contains("schema: 'semaprax.fused-source.v1'"));
        assert!(ENTRY_TEMPLATE.contains("const MAX_ACCOUNT_LIMIT = 1073741824;"));
        assert_eq!(MAX_ACCOUNT_LIMIT, 1_073_741_824);
        assert!(HOST.contains("const REGISTRY_ENTRY_BYTES = 8 + 4 * 6 + 1;"));
        assert_eq!(REGISTRY_ENTRY_BYTES, 33);
    });
}

#[test]
fn caller_inputs_and_ordinary_sources_refuse_before_any_artifact() {
    with_owner(|owner| {
        let funding = per_invocation(owner);
        let upper = source_sha256().to_uppercase();
        let long = format!("{}0", source_sha256());
        for digest in ["", "00", upper.as_str(), long.as_str()] {
            assert!(package_files(owner, funding, digest).is_err());
        }
        for funding in [0, (1 << 30) + 1, usize::MAX] {
            assert!(package_files(owner, funding, &source_sha256()).is_err());
        }
        assert!(package_files(owner, 1 << 30, &source_sha256()).is_ok());
    });
    // Counts within the unchanged 32-event cap never yield a selected owner,
    // so no fused package can exist for them; the default route keeps T267.
    let ordinary = ORIGINAL.replace("4096", "  32");
    let source = crate::parse(&ordinary, "fused-source-package-32.spx").unwrap();
    assert!(validate_selected(&source).is_err());
    assert!(crate::check(ORIGINAL, "fused-source-package-default.spx")
        .unwrap_err()
        .iter()
        .any(|diagnostic| diagnostic.code == "SPX-T267"));
}

#[test]
fn spliced_foreign_or_tampered_contents_refuse_before_publication() {
    with_owner(|owner| {
        let files = package_files(owner, per_invocation(owner), &source_sha256()).unwrap();
        let parent = scratch("splice");
        let mut splices: Vec<Vec<(&'static str, Vec<u8>)>> = Vec::new();
        let mut module = files.clone();
        *module[2].1.last_mut().unwrap() ^= 1;
        splices.push(module);
        let mut host = files.clone();
        host[3].1.push(b'\n');
        splices.push(host);
        let mut grant = files.clone();
        let edited = String::from_utf8(grant[0].1.clone())
            .unwrap()
            .replacen("\"rows\":4096", "\"rows\":4095", 1);
        grant[0].1 = edited.into_bytes();
        splices.push(grant);
        let mut entry = files.clone();
        entry[1].1 = ENTRY_TEMPLATE.replacen(MANIFEST_SLOT, "{}", 1).into_bytes();
        splices.push(entry);
        let mut reordered = files.clone();
        reordered.swap(0, 2);
        splices.push(reordered);
        let mut missing = files.clone();
        missing.pop();
        splices.push(missing);
        for (index, spliced) in splices.iter().enumerate() {
            let output = parent.join(format!("spliced-{index}"));
            assert!(publish(spliced, &output).is_err(), "splice {index} published");
            assert!(!output.exists(), "splice {index} left a destination");
        }
        std::fs::remove_dir_all(parent).unwrap();
    });
}

#[test]
fn facade_package_type_carries_the_same_owner_artifacts_to_publication() {
    with_owner(|owner| {
        let funding = per_invocation(owner);
        let expected = package_files(owner, funding, &source_sha256()).unwrap();
        let package = emit_fused_source_package(owner, funding, &source_sha256()).unwrap();
        let carried: Vec<(&str, Vec<u8>)> = package
            .files()
            .map(|(name, bytes)| (name, bytes.to_vec()))
            .collect();
        assert_eq!(carried, expected);
        let parent = scratch("facade");
        let output = parent.join("package");
        publish_fused_source_package(&package, &output).unwrap();
        for (name, bytes) in &expected {
            assert_eq!(&std::fs::read(output.join(name)).unwrap(), bytes);
        }
        // A fresh destination is required; republishing refuses.
        assert!(publish_fused_source_package(&package, &output).is_err());
        std::fs::remove_dir_all(parent).unwrap();
    });
}

#[test]
fn published_package_runs_original_with_exact_cumulative_and_one_short_funding() {
    with_owner(|owner| {
        let registry = registry_buffer(owner);
        let exact = per_invocation(owner);
        let parent = scratch("run");
        for (mode, funding) in [
            ("exact", registry + 3 * exact),
            ("one-short", registry + exact - 1),
            ("language", registry + 3 * exact),
            ("tampered", registry + exact),
        ] {
            let files = package_files(owner, funding, &source_sha256()).unwrap();
            let output = parent.join(mode);
            publish(&files, &output).unwrap();
            if mode == "tampered" {
                // Tamper with the published compiler-paired file itself; only
                // the entry's public fixed loader may observe it.
                let module = output.join(MODULE_FILE);
                let mut bytes = std::fs::read(&module).unwrap();
                *bytes.last_mut().unwrap() ^= 1;
                std::fs::write(&module, bytes).unwrap();
            }
            run_node(&output, mode);
        }
        std::fs::remove_dir_all(parent).unwrap();
    });
}
