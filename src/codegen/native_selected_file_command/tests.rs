use super::*;

#[test]
fn direct_selected_emitter_requires_both_declared_capabilities() {
    fn program(permits: &[&str]) -> crate::hir::ResolvedProgram {
        let declaration = if permits.is_empty() {
            String::new()
        } else {
            format!("permit {{ {} }}", permits.join(", "))
        };
        crate::hir::resolve(
            &crate::parse(
                &format!(
                    "module selected.direct;{declaration} @id(\"app.main\") fn main()->i64{{0}}"
                ),
                "selected-direct.spx",
            )
            .unwrap(),
        )
        .unwrap()
    }
    for permits in [
        vec![],
        vec!["fs.read"],
        vec!["process.args.read"],
        vec!["process.stderr.write"],
        vec!["fs.read", "process.stdout.write"],
        vec!["process.args.read", "process.stderr.write"],
    ] {
        let diagnostic = emit_hir_c_with_selected_file_command(&program(&permits)).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-T273");
        assert_eq!(diagnostic.message,
            "single-file command-line program authority mismatch: source-command.selected-file.v1 requires both fs.read and process.args.read");
    }
    let admitted =
        emit_hir_c_with_selected_file_command(&program(&["fs.read", "process.args.read"]));
    if cfg!(all(unix, target_pointer_width = "64")) {
        let source = admitted.unwrap();
        assert!(source.contains("spx_selected_file_launch_v1"));
    } else {
        assert_eq!(admitted.unwrap_err().code, "SPX-B104");
    }
}
