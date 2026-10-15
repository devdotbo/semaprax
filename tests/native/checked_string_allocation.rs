//! Private allocation-kernel physical evidence, not compiler/codec recovery.
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const RUNTIME: &str =
    include_str!("../../src/codegen/native_emit/checked_string_allocation/runtime.c");
const STDIO: &str = include_str!("../support/native_fixture_stdio.c");
const PROBE: &str = include_str!("checked_string_allocation/probe.c");

#[test]
fn private_checked_string_kernel_recovers_each_physical_allocation_at_o0_o2() {
    let compiler = std::env::var_os("CLANG").map_or_else(|| PathBuf::from("clang"), PathBuf::from);
    let root = std::env::temp_dir().join(format!(
        "semaprax-checked-string-kernel-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir(&root).unwrap();
    let source = root.join("probe.c");
    fs::write(&source, format!("{STDIO}\n{RUNTIME}\n{PROBE}")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let executable = root.join(format!(
            "probe{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let compiled = Command::new(&compiler)
            .current_dir(&root)
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&source)
            .arg("-o")
            .arg(&executable)
            .output()
            .expect("Clang is required for the selected physical allocation kernel gate");
        assert!(
            compiled.status.success(),
            "{}: {}",
            root.display(),
            String::from_utf8_lossy(&compiled.stderr)
        );
        let result = Command::new(&executable)
            .current_dir(&root)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{optimization}: stdout={} stderr={}",
            String::from_utf8_lossy(&result.stdout),
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(result.stdout, b"checked-string-kernel-v1: ok\n");
        assert!(result.stderr.is_empty());
    }
    fs::remove_dir_all(root).unwrap();
}
