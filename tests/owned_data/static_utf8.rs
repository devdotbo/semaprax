//! Internal static text carrier gates; public export profiles stay closed.
use semaprax::{codegen, hir, parse, wasm};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static SERIAL: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = r#"module static.backend;
@id("static.read")
fn read(text: borrow str, index: usize) -> i64 {
    match str_byte_at(text, index) {
        Option::Some { value: byte } => i64_from_u8(byte),
        Option::None {} => -1,
    }
}
@id("static.length")
fn text_length(text: borrow str) -> i64 { str_len_bytes(text) }
@id("static.scalar")
fn scalar() -> i64 { 42 }
@id("app.main")
fn main() -> i64 {
    let key = str"é\u{0}\"\\";
    let bytes = str_as_bytes(key);
    let empty = str"";
    let mut index = 0usize;
    let mut total = 0;
    while index < 5usize {
        total = total + read(key, index);
        index = index + 1usize;
        0
    }
    if total == 490 && str_len_bytes(key) == 5
        && byte_len(bytes) == 5usize && str_is_empty(empty)
        && str_contains(key, str"\u{0}\"")
        && str_starts_with(key, str"é")
        && read(key, 5usize) == -1
        && read(key, 18446744073709551615usize) == -1
        && read(empty, 0usize) == -1 { 42 } else { -1 }
}
"#;

fn directory() -> std::path::PathBuf {
    let serial = SERIAL.fetch_add(1, Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!(
        "semaprax-static-utf8-{}-{serial}",
        std::process::id()
    ));
    std::fs::create_dir(&path).unwrap();
    path
}

#[test]
fn native_static_utf8_reads_repeat_without_any_heap_constructor_at_o0_o2() {
    let ast = parse(SOURCE, Path::new("static-native.spx")).unwrap();
    let resolved = hir::resolve(&ast).unwrap();
    hir::validate(&resolved).unwrap();
    assert!(resolved
        .functions
        .iter()
        .all(|f| f.cleanup_plan.slots.is_empty()));
    let generated = codegen::emit_c(&ast).unwrap();
    assert_eq!(generated, codegen::emit_c(&ast).unwrap());
    assert!(generated.contains("\\303\\251\\000"));
    assert!(!generated.contains("spx_string_from_literal"));
    assert!(!generated.contains("spx_string_drop"));
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let path = directory();
    let preamble = include_str!("static_utf8/allocations.c");
    let probe = include_str!("static_utf8/probe.c");
    let input = path.join("probe.c");
    std::fs::write(&input, format!("{preamble}\n{generated}\n{probe}")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let executable = path.join(format!(
            "probe{}{}",
            optimization,
            std::env::consts::EXE_SUFFIX
        ));
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&input)
            .arg("-o")
            .arg(&executable)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&executable).output().unwrap();
        assert!(
            executed.status.success(),
            "static literal probe {optimization}: {:?} {}",
            executed.status.code(),
            String::from_utf8_lossy(&executed.stderr)
        );
    }
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn wasm_static_utf8_pool_reads_repeat_without_string_or_bytes_allocation() {
    let ast = parse(SOURCE, Path::new("static-wasm.spx")).unwrap();
    let bytes = wasm::emit_module(&ast).unwrap();
    assert_eq!(bytes, wasm::emit_module(&ast).unwrap());
    wasmparser::Validator::new().validate_all(&bytes).unwrap();
    if Command::new("node").arg("--version").output().is_err() {
        return;
    }
    let path = directory();
    std::fs::write(path.join("probe.wasm"), bytes).unwrap();
    std::fs::write(
        path.join("probe.mjs"),
        include_str!("static_utf8/probe.mjs"),
    )
    .unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(&path)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::remove_dir_all(path).unwrap();
}

#[test]
fn static_utf8_backends_replay_types_producers_and_keep_public_providers_closed() {
    let ast = parse(SOURCE, Path::new("static-hostile.spx")).unwrap();
    let resolved = hir::resolve(&ast).unwrap();
    for forged in 0..4 {
        let mut candidate = resolved.clone();
        let main = candidate
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        let hir::ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
            panic!("block")
        };
        let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
            panic!("literal")
        };
        match forged {
            0 => value.ty = hir::ResolvedType::String,
            1 => value.ownership = hir::OwnershipMode::Own,
            2 => value.id = hir::ExpressionId::new("forged.static.producer"),
            _ => value.kind = hir::ResolvedExprKind::String("owned".to_owned()),
        }
        assert_eq!(
            codegen::emit_hir_c(&candidate).unwrap_err().code,
            "SPX-H006"
        );
        assert_eq!(
            wasm::emit_resolved_module(&candidate).unwrap_err().code,
            "SPX-H006"
        );
    }
    assert_eq!(
        wasm::emit_module_with_scalar_exports(&ast, &["static.scalar".to_owned()])
            .unwrap_err()
            .code,
        "SPX-W119"
    );
    assert_eq!(
        wasm::emit_module_with_text_exports(&ast, &["static.length".to_owned()])
            .unwrap_err()
            .code,
        "SPX-W119"
    );
}
