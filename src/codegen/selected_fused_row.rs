//! Private selected FusedConstructionV1 native lowering and runtime.
//!
//! The production-shaped route is [`emit_validated`], the explicit accepting
//! API accepts only the full immutable `ValidatedFusedSelection` owner.
//! The default compiler does not select it; native qualification is pending.
//! The old `construction_fusion` descriptor reaches the same lowering only
//! through the test-only UNVALIDATED precursor, which carries no artifact
//! authority. No default emitter, manifest, name, cached flag or graph fact
//! selects either route, and the original 4096-row program still refuses in
//! ordinary source verification. The explicit owner route has authored gates for
//! that immutable original; execution qualification remains pending.
use crate::diagnostic::Diagnostic;
mod cleanup;
mod crossing;
mod emit;
mod layout;
mod lower;
#[cfg(test)]
mod precursor;
mod selection;
#[cfg(test)]
mod tests;

#[allow(
    unused_imports,
    reason = "private receiving API types are retained for explicit engine callers"
)]
pub(crate) use emit::{emit_validated, Artifact, ENTRY};
#[allow(
    unused_imports,
    reason = "private receiving API types are retained for explicit engine callers"
)]
pub(crate) use selection::CarrierFacts;
pub(super) const RUNTIME: &str = include_str!("selected_fused_row/runtime.c");

fn error(message: &str) -> Diagnostic {
    Diagnostic::io("SPX-B104", format!("selected fused-row profile: {message}"))
}

#[cfg(test)]
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
            "{}\n{}\n{}",
            include_str!("selected_fused_row/probe_prefix.c"),
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
