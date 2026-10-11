use super::*;
const SUCCESS: &str = r#"module bounded;
@id("allocate") fn allocate(size:usize)->Bytes{bytes_zeroed_bounded(size,64usize)}
@id("allocate.after") fn after(size:usize)->Bytes{let kept=bytes_zeroed(1usize);allocate(size)}
@id("buffer.main") fn main()->i64{let empty=allocate(0usize);let bytes=after(64usize);if byte_len(bytes_as_slice(empty))==0usize && byte_len(bytes_as_slice(bytes))==64usize{737}else{0}}
"#;
#[test]
fn exact_bounded_allocation_preserves_source_hir_graph_and_dynamic_failure() {
    let program = parse(SUCCESS, "bounded.spx").unwrap();
    assert!(verify::verify(&program).is_empty());
    let resolved = hir::resolve(&program).unwrap();
    hir::validate(&resolved).unwrap();
    let canonical = format::canonical(&program);
    let reparsed = parse(&canonical, "roundtrip.spx").unwrap();
    assert_eq!(
        graph::to_json(&program).unwrap(),
        graph::to_json(&reparsed).unwrap()
    );
    assert_eq!(
        codegen::emit_c(&program).unwrap(),
        codegen::emit_c(&reparsed).unwrap()
    );
    assert_eq!(
        wasm::emit_module(&program).unwrap(),
        wasm::emit_module(&reparsed).unwrap()
    );
    let success: serde_json::Value =
        serde_json::from_str(&interpret(SUCCESS, "bounded-success")).unwrap();
    assert_eq!(success["payload"]["outcome"]["value"], "737");
    for requested in [65usize, usize::MAX] {
        let failed = SUCCESS.replace("after(64usize)", &format!("after({requested}usize)"));
        let envelope: serde_json::Value =
            serde_json::from_str(&interpret(&failed, "bounded-failure")).unwrap();
        let status = &envelope["payload"]["outcome"]["status"];
        assert_eq!(status["domain_id"], "semaprax.byte-buffer.v1");
        assert_eq!(status["code"], 2);
        assert_eq!(status["class"], "adapter");
    }
}
#[test]
fn bounded_allocation_keeps_literal_bound_and_legacy_closed_rules() {
    for body in [
        "let bound=4usize;let bytes=bytes_zeroed_bounded(1usize,bound);0",
        "let bytes=bytes_zeroed_bounded(1usize,131073usize);0",
        "let capacity=1usize;let bytes=bytes_zeroed(capacity);0",
    ] {
        assert_rejected(&program_source(body), "SPX-T271");
    }
    assert!(!error_codes(&program_source(
        "let bytes=bytes_zeroed_bounded(1usize,0usize);0"
    ))
    .contains(&"SPX-T271"));
    let in_loop = program_source(
        "let mut at=0usize;while at<1usize{let bytes=bytes_zeroed_bounded(at,64usize);at=at+1usize;0}0",
    );
    let errors = error_codes(&in_loop);
    assert!(errors.contains(&"SPX-T267") || errors.contains(&"SPX-T252"));
    let array_view_loop = program_source(
        "let key=[65u8,66u8];let mut at=0usize;while at<2usize{let byte=byte_get(array_as_slice(key),at);at=at+1usize;0}i64_from_usize(at)",
    );
    let checked = parse(&array_view_loop, "bounded-key-view.spx").unwrap();
    assert!(verify::verify(&checked).is_empty());
    hir::validate(&hir::resolve(&checked).unwrap()).unwrap();
    let reserved =
        "module bad;@id(\"spoof\") fn bytes_zeroed_bounded(size:usize,bound:usize)->i64{0}";
    assert!(error_codes(reserved).contains(&"SPX-S113"));
}
#[test]
fn bounded_native_allocation_checks_before_allocator_and_preserves_poisoned_result() {
    if !command_available("clang") {
        return;
    }
    let generated = codegen::emit_c(&parse(SUCCESS, "bounded-native.spx").unwrap()).unwrap();
    let observer = include_str!("../../native_owned_utf8_settlement_v1/allocations.c");
    let symbol = format!(
        "spx_decl_{}",
        "allocate"
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let after = format!(
        "spx_decl_{}",
        "allocate.after"
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let probe = include_str!("bounded_allocation.c");
    let source = format!(
        "#define FIXTURE_TRACK_CALLOC\n{observer}\n#define main fixture_generated_main\n{generated}\n#undef main\n#undef malloc\n#undef calloc\n#undef free\n#define ALLOCATE {symbol}\n#define AFTER {after}\n{probe}"
    );
    let root = std::env::temp_dir().join(format!("bounded-native-{}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    for optimization in ["-O0", "-O2"] {
        let c = root.join("probe.c");
        let binary = root.join(format!("probe{optimization}"));
        std::fs::write(&c, &source).unwrap();
        let built = Command::new("clang")
            .args(["-std=c11", "-Wall", "-Wextra", "-Werror", optimization])
            .arg(&c)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            built.status.success(),
            "{}",
            String::from_utf8_lossy(&built.stderr)
        );
        let result = Command::new(binary).output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
