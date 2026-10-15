use super::*;

fn program(permits: &[&str]) -> crate::hir::ResolvedProgram {
    let declaration = if permits.is_empty() {
        String::new()
    } else {
        format!("permit {{ {} }}", permits.join(", "))
    };
    crate::hir::resolve(
        &crate::parse(
            &format!(
        "module selected.resource.direct;{declaration} @id(\"app.main\") fn main()->i64{{0}}"
    ),
            "selected-resource-direct.spx",
        )
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn direct_resource_emitter_requires_both_declared_capabilities() {
    for permits in [
        vec![],
        vec!["fs.read"],
        vec!["process.args.read"],
        vec!["process.stderr.write"],
        vec!["fs.read", "process.stdout.write"],
        vec!["process.args.read", "process.stderr.write"],
        vec!["process.args.read", "process.stdout.write"],
    ] {
        let diagnostic =
            emit_hir_c_with_selected_file_resource_command(&program(&permits)).unwrap_err();
        assert_eq!(diagnostic.code, "SPX-T273");
        assert_eq!(diagnostic.message, "single-file command-line program authority mismatch: source-command.selected-file.resource-output.v1 requires both fs.read and process.args.read");
    }
    for permits in [
        vec!["fs.read", "process.args.read"],
        vec![
            "fs.read",
            "process.args.read",
            "process.stderr.write",
            "process.stdout.write",
        ],
    ] {
        let result = emit_hir_c_with_selected_file_resource_command(&program(&permits));
        if cfg!(all(unix, target_pointer_width = "64")) {
            let source = result.unwrap();
            assert!(source.contains("#define SPX_COMMAND_ARGUMENT_LIMIT_V1 UINT32_C(256)"));
            assert!(source.contains("#define SPX_COMMAND_OUTPUT_CAPACITY_V1 UINT64_C(1048576)"));
            assert!(source.contains("spx_selected_file_launch_v1"));
        } else {
            assert_eq!(result.unwrap_err().code, "SPX-B104");
        }
    }
    let broader = program(&["process.args.read", "process.stdout.write"]);
    assert!(crate::codegen::emit_hir_c_with_source_command(&broader).is_ok());
    assert!(crate::codegen::emit_hir_c_with_source_resource_command(&broader).is_ok());
}
