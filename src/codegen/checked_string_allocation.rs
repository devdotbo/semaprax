//! Test-owned native emission precursor, never a production output profile.
//! The explicit kernel-only selector cannot supply source/cleanup authority.
const KERNEL: &str = include_str!("native_emit/checked_string_allocation/runtime.c");
const ADAPTER: &str = include_str!("native_emit/checked_string_allocation/status_adapter.c");
fn emit_private_kernel_adapter() -> String {
    let mut output = String::from("#define SPX_CHECKED_STRING_KERNEL_ONLY_V1 1\n");
    super::native_runtime::emit_status_runtime(&mut output);
    output.push_str(KERNEL);
    output.push_str(ADAPTER);
    output
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;
    use std::process::Command;
    use std::sync::atomic::{AtomicU64, Ordering};
    static SERIAL: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn private_adapter_emission_retains_exact_u32_status_and_nine_borrowed_constructors() {
        let emitted = emit_private_kernel_adapter();
        assert_eq!(emitted, emit_private_kernel_adapter());
        assert!(emitted.contains("typedef uint32_t spx_status_token;"));
        assert!(emitted.starts_with("#define SPX_CHECKED_STRING_KERNEL_ONLY_V1 1\n"));
        for operation in [
            "from_literal",
            "clone",
            "from_char",
            "from_i64",
            "from_usize",
            "from_str",
            "from_utf8",
            "concat",
            "collection_clone",
        ] {
            assert_eq!(
                emitted
                    .matches(&format!("spx_checked_string_adapter_{operation}_v1("))
                    .count(),
                1
            );
        }
        assert!(!ADAPTER.contains("malloc("));
        assert!(!ADAPTER.contains("free("));
        assert!(!ADAPTER.contains("spx_string_drop("));
    }

    #[test]
    fn private_adapter_uses_real_arena_tokens_and_preserves_fault_settlement_at_o0_o2() {
        let compiler =
            std::env::var_os("CLANG").map_or_else(|| PathBuf::from("clang"), PathBuf::from);
        let root = std::env::temp_dir().join(format!(
            "checked-string-status-adapter-{}-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos(),
            SERIAL.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&root).unwrap();
        let source = root.join("probe.c");
        std::fs::write(
            &source,
            format!(
                "{}\n{}\n{}",
                include_str!("../../tests/support/native_fixture_stdio.c"),
                emit_private_kernel_adapter(),
                include_str!("native_emit/checked_string_allocation/status_adapter_probe.c")
            ),
        )
        .unwrap();
        for optimization in ["-O0", "-O2"] {
            let binary = root.join(format!(
                "probe{optimization}{}",
                std::env::consts::EXE_SUFFIX
            ));
            let compiled = Command::new(&compiler)
                .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
                .arg(&source)
                .arg("-o")
                .arg(&binary)
                .output()
                .expect("Clang is required for this selected private adapter gate");
            assert!(
                compiled.status.success(),
                "{}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            let result = Command::new(&binary).output().unwrap();
            assert!(
                result.status.success(),
                "{}",
                String::from_utf8_lossy(&result.stderr)
            );
            assert_eq!(result.stdout, b"checked-string-status-adapter-v1: ok\n");
            assert!(result.stderr.is_empty());
        }
        std::fs::remove_dir_all(root).unwrap();
    }
}
