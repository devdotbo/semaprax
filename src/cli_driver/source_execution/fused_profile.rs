//! Explicit profile dispatch occurs before ordinary source verification.
use super::*;
fn source(path: &Path, json: bool) -> Result<String, u8> {
    std::fs::read_to_string(path).map_err(|error| {
        report(
            &[Diagnostic::io(
                "SPX-I001",
                format!("cannot read {}: {error}", path.display()),
            )],
            json,
        )
    })
}
pub(super) fn run(path: &Path, options: &cli::execution::ExecutionOptions) -> Result<(), u8> {
    let account = options
        .fused_account
        .expect("validated explicit profile account");
    let text = source(path, options.json)?;
    if !options.native {
        let result = semaprax::fused_source::run_interpreter(
            &text,
            path,
            account,
            options.max_steps.unwrap_or(interpreter::DEFAULT_MAX_STEPS),
        )
        .map_err(|errors| report(&errors, options.json))?;
        println!("{}", result.envelope());
        return result.returned().then_some(()).ok_or(1);
    }
    let c = semaprax::fused_source::emit_native(&text, path, account)
        .map_err(|errors| report(&errors, options.json))?;
    let mut scratch =
        native_scratch::Scratch::create(&format!("program{}", std::env::consts::EXE_SUFFIX), None)
            .map_err(|error| {
                report(
                    &[Diagnostic::io(
                        "SPX-I101",
                        format!("cannot create fused run scratch: {error}"),
                    )],
                    options.json,
                )
            })?;
    let result = (|| {
        codegen::compile_native_executable(&c, scratch.path())
            .map_err(|error| report(&[error], options.json))?;
        scratch.seal().map_err(|error| {
            report(
                &[Diagnostic::io(
                    "SPX-I101",
                    format!("cannot seal fused run scratch: {error}"),
                )],
                options.json,
            )
        })?;
        let status = Command::new(scratch.path()).status().map_err(|error| {
            report(
                &[Diagnostic::io(
                    "SPX-I101",
                    format!("cannot run fused artifact: {error}"),
                )],
                options.json,
            )
        })?;
        match child_result_code(&status) {
            0 => Ok(()),
            code => Err(code),
        }
    })();
    // Cleanup never replaces a selected child status. No refused program gains
    // scratch authority: emission and full proof precede Scratch::create.
    let _ = scratch.cleanup();
    result
}
pub(super) fn build(options: &cli::build::BuildOptions, input: &Path) -> Result<(), u8> {
    let account = options
        .fused_account
        .expect("validated explicit profile account");
    let text = source(input, options.json)?;
    let output = options.output.as_deref().expect("source output");
    match options.target.as_str() {
        "native" => {
            let c = semaprax::fused_source::emit_native(&text, input, account)
                .map_err(|errors| report(&errors, options.json))?;
            let mut destination = cli::build::SourceNativeOutput::prepare(output)
                .map_err(|error| report(&[error], options.json))?;
            let mut scratch = native_scratch::Scratch::create(
                &format!("program{}", std::env::consts::EXE_SUFFIX),
                None,
            )
            .map_err(|error| {
                report(
                    &[Diagnostic::io(
                        "SPX-I301",
                        format!("cannot create fused build scratch: {error}"),
                    )],
                    options.json,
                )
            })?;
            let result = (|| {
                codegen::compile_native_executable(&c, scratch.path())
                    .map_err(|error| report(&[error], options.json))?;
                scratch.seal().map_err(|error| {
                    report(
                        &[Diagnostic::io(
                            "SPX-I301",
                            format!("cannot seal fused build scratch: {error}"),
                        )],
                        options.json,
                    )
                })?;
                destination
                    .publish(scratch.path())
                    .map_err(|error| report(&[error], options.json))
            })();
            let _ = scratch.cleanup();
            result?;
        }
        "wasm" => {
            let package = semaprax::fused_source::emit_wasm_package(&text, input, account)
                .map_err(|errors| report(&errors, options.json))?;
            package
                .publish(output)
                .map_err(|error| report(&[error], options.json))?;
        }
        _ => unreachable!("validated fused target"),
    }
    report_source_build_success(options, "fused-construction-v1 artifact", output, None);
    Ok(())
}
