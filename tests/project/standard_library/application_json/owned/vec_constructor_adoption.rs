//! Application source differential: exact pre-constructor test modules remain
//! independent controls. Existing corpus and old-syntax refusal gates are separate.
use super::public_example::{compile_native, execute};
use super::*;
use std::path::Path;

pub(super) fn assert_application_control(root: &Path, adopted: &str, original: &str) {
    for (route, source) in [("constructor", adopted), ("explicit-chain", original)] {
        let canonical_source = canonical(source);
        assert_eq!(canonical(&canonical_source), canonical_source, "{route}");
        std::fs::write(root.join("src/tests.spx"), canonical_source).unwrap();
        let c = project::with_authenticated_project(&root.join("semaprax.toml"), |snapshot| {
            semaprax::hir::validate(snapshot.test_program()).map_err(|error| vec![error])?;
            // Reentry rechecks both independently authored application cases.
            for _ in 0..2 {
                let tests = snapshot.execute_test(&project::ProjectExecutionOptions::default())?;
                assert!(tests.command_succeeded(), "{route}: {tests:?}");
                assert_eq!(tests.cases().len(), 2, "{route}");
            }
            codegen::emit_hir_c(snapshot.test_program()).map_err(|error| vec![error])
        })
        .unwrap();
        for optimization in ["-O0", "-O2"] {
            let binary = compile_native(&c, root, optimization);
            for _ in 0..2 {
                let output = execute(&binary, &[]);
                assert!(
                    output.status.success(),
                    "{route} {optimization}: {output:?}"
                );
                assert_eq!(output.stdout, b"0\n", "{route} {optimization}");
                assert!(
                    output.stderr.is_empty(),
                    "{route} {optimization}: {output:?}"
                );
            }
        }
    }
}
