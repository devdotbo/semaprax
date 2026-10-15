//! Selected-file v34 preserves raw option order and v28 text/output resources.
use super::{project, Command, Fixture, ProjectExecutionOptions, RESOURCE_MANIFEST};

const APP: &str = r#"module decimal.command;
permit { fs.read, process.args.read, process.stderr.write, process.stdout.write }

@id("decimal.command.main")
fn main() -> i64 uses { fs.read, process.args.read, process.stderr.write, process.stdout.write }
{
    if args_len() != 34usize {
        let usage = "usage\n";
        let view = string_as_str(usage);
        let ignored = stderr_write(str_as_bytes(view));
        2
    } else {
        let path = arg_utf8(0usize);
        let left = file_read_text(path);
        let right = file_read_text(path);
        let joined = string_concat(left, right);
        let view = string_as_str(joined);
        let bytes = str_as_bytes(view);
        let last_view = arg_utf8(33usize);
        let last = string_from_str(last_view);
        if last != "last" { 2 } else {
            let written = stdout_write(bytes);
            0
        }
    }
}
"#;

fn manifest() -> String {
    RESOURCE_MANIFEST
        .replace(
            "source-command.resource-output.v1",
            "source-command.selected-file.resource-output.v1",
        )
        .replace(
            "argv-utf8+file-text.v1",
            "argv-utf8-256+selected-file.follow.v1",
        )
}

fn compile(fixture: &Fixture, opt: &str, resource: bool) -> std::path::PathBuf {
    let source =
        project::with_authenticated_project(&fixture.0.join("semaprax.toml"), |snapshot| {
            let graph: serde_json::Value = serde_json::from_str(snapshot.semantic_graph()).unwrap();
            assert_eq!(graph["project_schema"], "semaprax.project.v34");
            assert_eq!(
                snapshot
                    .execute_entry(&ProjectExecutionOptions::default())
                    .unwrap_err()[0]
                    .code,
                "SPX-F102"
            );
            assert_eq!(snapshot.test_wasm_module().unwrap_err()[0].code, "SPX-W120");
            let lock: serde_json::Value =
                serde_json::from_str(&project::render_project_lock(snapshot)?).unwrap();
            assert_eq!(
                lock["payload"]["package"]["contract"],
                "semaprax.project.v34"
            );
            let program = snapshot.entry_program();
            let source = if resource {
                semaprax::codegen::emit_hir_c_with_selected_file_resource_command(program)
            } else {
                semaprax::codegen::emit_hir_c_with_selected_file_command(program)
            };
            source.map_err(|error| vec![error])
        })
        .unwrap();
    let c = fixture.0.join(format!("resource-{opt}-{resource}.c"));
    let binary = fixture.0.join(format!("resource-{opt}-{resource}"));
    std::fs::write(&c, source).unwrap();
    let output = Command::new("clang")
        .args(["-std=c11", "-Wall", "-Wextra", "-Werror", opt])
        .arg(&c)
        .arg("-o")
        .arg(&binary)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    binary
}

#[test]
fn raw_options_and_large_output_are_additive_at_o0_o2() {
    let fixture = Fixture::new(APP, &manifest());
    let payload = vec![b'9'; 33_000];
    std::fs::write(fixture.0.join("input"), &payload).unwrap();
    for opt in ["-O0", "-O2"] {
        let mut args = vec!["--spx-selected-file-v1", "0", "--", "input"];
        args.extend(["--json"; 32]);
        args.push("last");
        let binary = compile(&fixture, opt, true);
        let output = Command::new(binary)
            .current_dir(&fixture.0)
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(
            output.status.code(),
            Some(0),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert_eq!(
            output.stdout,
            [payload.as_slice(), payload.as_slice()].concat()
        );
        assert!(output.stderr.is_empty());
        // Exact v33 control: its 16-argv bound refuses before any file read.
        let frozen = compile(&fixture, opt, false);
        let refused = Command::new(frozen)
            .current_dir(&fixture.0)
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(refused.status.code(), Some(2));
        assert!(refused.stdout.is_empty());
        assert_eq!(refused.stderr, b"SEMAPRAX selected-file command: optional --spx-selected-file-v1 INDEX -- precedes unchanged application arguments (INDEX 0..15)\n");
    }
}

#[test]
fn resource_profile_requires_its_exact_input_and_authority() {
    for wrong in [
        manifest().replace(
            "argv-utf8-256+selected-file.follow.v1",
            "argv-utf8+selected-file.follow.v1",
        ),
        manifest().replace("\"fs.read\", ", ""),
        manifest().replace("matrix = [\"native64\"]", "matrix = [\"wasm32\"]"),
    ] {
        assert_eq!(
            super::ProjectManifest::parse(&wrong).unwrap_err()[0].code,
            "SPX-J100"
        );
    }
}

#[test]
fn expanded_argument_and_total_byte_bounds_remain_exact_at_o0_o2() {
    let app = APP
        .replace("args_len() != 34usize", "args_len() != 256usize")
        .replace(
            "let path = arg_utf8(0usize);",
            "let path = arg_utf8(255usize);",
        )
        .replace("arg_utf8(33usize)", "arg_utf8(254usize)");
    let fixture = Fixture::new(&app, &manifest());
    std::fs::write(fixture.0.join("input"), "x").unwrap();
    for opt in ["-O0", "-O2"] {
        let binary = compile(&fixture, opt, true);
        let mut args = vec![
            "--spx-selected-file-v1".to_owned(),
            "255".to_owned(),
            "--".to_owned(),
        ];
        args.extend(std::iter::repeat_n("--json".to_owned(), 254));
        args.extend(["last".to_owned(), "input".to_owned()]);
        let output = Command::new(&binary)
            .current_dir(&fixture.0)
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, b"xx");
        assert!(output.stderr.is_empty());
        args.push("extra".to_owned());
        let over = Command::new(&binary)
            .current_dir(&fixture.0)
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(over.status.code(), Some(2));
        assert!(over.stdout.is_empty());
        args.pop();
        // Sum all256 application arguments; the prefix is excluded.
        let total: usize = args[4..].iter().map(String::len).sum();
        args[3] = "a".repeat(65_536 - total);
        let exact = Command::new(&binary)
            .current_dir(&fixture.0)
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(exact.status.code(), Some(0));
        assert_eq!(exact.stdout, b"xx");
        assert!(exact.stderr.is_empty());
        args[3].push('a');
        let overflow = Command::new(&binary)
            .current_dir(&fixture.0)
            .args(&args)
            .output()
            .unwrap();
        assert_eq!(overflow.status.code(), Some(2));
        assert!(overflow.stdout.is_empty());
        assert_eq!(overflow.stderr, b"SEMAPRAX selected-file command: optional --spx-selected-file-v1 INDEX -- precedes unchanged application arguments (INDEX 0..255)\n");
    }
}
