//! Native Unix invocation-selected input; no relative-root provider reuse.
use super::COutput;

pub(super) fn emit_runtime(output: &mut impl COutput, _program: &crate::hir::ResolvedProgram) {
    super::native_host_output::emit_language_command_runtime(output);
    super::native_command_io::emit_line_runtime(output);
    output.push_str(include_str!("native_selected_file_command/runtime.c"));
}
pub(super) fn emit_process_adapter(output: &mut impl COutput, root_symbol: &str) {
    output.push_str(include_str!("native_selected_file_command/launcher.c"));
    writeln!(output, "#ifndef SPX_NO_ENTRY_WRAPPER\nint main(int argc, char **argv) {{ return spx_selected_file_launch_v1(argc, argv, {root_symbol}); }}\n#endif")
        .expect("writing selected file launcher cannot fail");
}

/// Emit only the supported native Unix adapter; no dormant target support.
pub fn emit_hir_c_with_selected_file_command(
    program: &crate::hir::ResolvedProgram,
) -> Result<String, crate::diagnostic::Diagnostic> {
    crate::hir::validate(program)?;
    if ![
        crate::string_ops::FILE_READ_TEXT_EFFECT,
        crate::command_io_ops::ARGS_READ_EFFECT,
    ]
    .iter()
    .all(|required| {
        program
            .permits
            .iter()
            .any(|permit| permit.as_str() == *required)
    }) {
        return Err(crate::diagnostic::Diagnostic::io(
            "SPX-T273",
            "single-file command-line program authority mismatch: source-command.selected-file.v1 requires both fs.read and process.args.read",
        ));
    }
    crate::source_command::validate_authority(program)?;
    if !cfg!(all(unix, target_pointer_width = "64")) {
        return Err(crate::diagnostic::Diagnostic::io(
            "SPX-B104",
            "source-command.selected-file.v1 requires a native64 Unix host",
        ));
    }
    super::reject_native_rust_for_native(program)?;
    super::emit_hir_c_with_labels(
        program,
        &std::collections::HashMap::new(),
        super::NativeOutputProfile::SourceSelectedFileCommand,
        None,
    )
}

#[cfg(test)]
mod tests;
