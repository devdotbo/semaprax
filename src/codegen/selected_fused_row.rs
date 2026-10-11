//! Private executable runtime precursor; no source/admission/emission selector.
//! The C owning probe supplies metadata and allocator authority explicitly.
#[test]
fn private_domain_affine_leases_fault_cleanup_and_reentry_at_o0_o2() {
    use std::process::Command;
    let root = std::env::temp_dir().join(format!(
        "fused-row-domain-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir(&root).unwrap();
    let source = root.join("probe.c");
    std::fs::write(
        &source,
        format!(
            "{}\n{}",
            include_str!("selected_fused_row/runtime.c"),
            include_str!("selected_fused_row/probe.c")
        ),
    )
    .unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!(
            "probe{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        let compiled = Command::new(std::env::var_os("CLANG").unwrap_or_else(|| "clang".into()))
            .args(["-std=c11", optimization, "-Wall", "-Wextra", "-Werror"])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let run = Command::new(&binary).output().unwrap();
        assert!(
            run.status.success(),
            "status {:?}: {}",
            run.status.code(),
            String::from_utf8_lossy(&run.stderr)
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
