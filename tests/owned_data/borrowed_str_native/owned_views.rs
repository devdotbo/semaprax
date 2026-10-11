//! Exact owned extent and separately bounded foreign ingress.
use super::{command_available, symbol, NEXT_ID};
use semaprax::{codegen, parse};
use std::path::Path;
use std::process::Command;
use std::sync::atomic::Ordering;

const SOURCE: &str = r#"module text.owned_extent;
@id("text.observe")
fn observe(value: borrow str) -> i64 {
    let bytes = str_as_bytes(value);
    if byte_len(bytes) == 131072usize && str_len_bytes(value) == 131072
        && str_contains(value, value) && str_starts_with(value, value) {
        42
    } else { -1 }
}
@id("text.pair")
fn pair(first: borrow str, second: borrow str) -> bool {
    str_len_bytes(first) == str_len_bytes(second)
}
@id("app.main")
fn main() -> i64 {
    let raw = bytes_zeroed(131072usize);
    let text = string_from_utf8(bytes_as_slice(raw));
    let view = string_as_str(text);
    observe(view)
}
"#;

#[test]
fn owned_long_str_and_byte_views_preserve_exact_extent_and_backend_settlement() {
    super::super::bulk_utf8::run(SOURCE, 42, None, -1);
    // String concatenation has no Slice extent grant. Its Str-only consumers
    // must preserve the full owned length, including all embedded NUL bytes.
    let larger = r#"module text.larger;
@id("text.forward") fn forward(value: borrow str) -> bool {
    !str_is_empty(value) && str_len_bytes(value) == 262144
        && str_starts_with(value, value) && str_contains(value, value)
}
@id("app.main") fn main() -> i64 {
    let first = bytes_zeroed(131072usize);
    let second = bytes_zeroed(131072usize);
    let left = string_from_utf8(bytes_as_slice(first));
    let right = string_from_utf8(bytes_as_slice(second));
    let whole = string_concat(left, right);
    let view = string_as_str(whole);
    if forward(view) { 43 } else { -1 }
}"#;
    super::super::bulk_utf8::run(larger, 43, None, -1);
}

fn execute(generated: &str, probe: &str, optimization: &str) -> std::process::Output {
    let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("owned-str-extent-{}-{id}", std::process::id()));
    std::fs::create_dir_all(&root).unwrap();
    let source = root.join("probe.c");
    let executable = root.join("probe");
    std::fs::write(&source, format!("{generated}\n{probe}")).unwrap();
    let compiled = Command::new("clang")
        .args([
            "-std=c11",
            optimization,
            "-Wall",
            "-Wextra",
            "-Werror",
            "-DSPX_NO_ENTRY_WRAPPER",
        ])
        .arg(&source)
        .arg("-o")
        .arg(&executable)
        .output()
        .unwrap();
    assert!(
        compiled.status.success(),
        "{}",
        String::from_utf8_lossy(&compiled.stderr)
    );
    let output = Command::new(&executable).output().unwrap();
    std::fs::remove_dir_all(root).unwrap();
    output
}

#[test]
fn selected_owned_view_runtime_keeps_foreign_root_and_slice_bounds_at_o0_o2() {
    if !command_available("clang") {
        return;
    }
    let ast = parse(SOURCE, Path::new("owned-str-extent.spx")).unwrap();
    let generated = codegen::emit_c(&ast).unwrap();
    assert!(generated.contains("spx_str_require_root_valid(spx_param_0)"));
    assert!(generated.contains("Critical factorization and Two-Way"));
    assert!(!generated.contains("uint16_t prefix[SPX_BORROWED_STR_MAX_BYTES]"));
    assert!(generated.contains("spx_slice_u8_require_owned_view_valid("));
    let ingress = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[16]; struct spx_context ctx = {{0}};
    if (!spx_context_init(&ctx, UINT64_C(9), entries, UINT32_C(16), NULL, NULL, NULL)) return 10;
    int64_t result = -1;
    /* Extent must refuse before the deliberately unreadable payload is used. */
    spx_str_v1 foreign = {{NULL, UINT64_C(65537)}};
    (void){observe}(&ctx, foreign, &result);
    return 0;
}}
"#,
        observe = symbol("text.observe")
    );
    let cumulative = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[16]; struct spx_context ctx = {{0}};
    if (!spx_context_init(&ctx, UINT64_C(9), entries, UINT32_C(16), NULL, NULL, NULL)) return 10;
    static uint8_t bytes[40000]; bool observed = false;
    spx_str_v1 value = {{bytes, UINT64_C(40000)}};
    (void){pair}(&ctx, value, value, &observed);
    return 0;
}}
"#,
        pair = symbol("text.pair")
    );
    let slice = r#"int main(void) {
    struct spx_status_entry entries[16]; struct spx_context ctx = {0};
    if (!spx_context_init(&ctx, UINT64_C(9), entries, UINT32_C(16), NULL, NULL, NULL)) return 10;
    spx_slice_u8_v1 view = {NULL, UINT64_C(131073)};
    spx_slice_u8_require_owned_view_valid(view); return 0;
}"#;
    for optimization in ["-O0", "-O2"] {
        let refusal = execute(&generated, &ingress, optimization);
        assert!(!refusal.status.success());
        assert!(String::from_utf8_lossy(&refusal.stderr)
            .contains("borrowed str length exceeds native profile"));
        let refusal = execute(&generated, &cumulative, optimization);
        assert!(!refusal.status.success());
        assert!(String::from_utf8_lossy(&refusal.stderr)
            .contains("borrowed invocation exceeds cumulative root byte budget"));
        let refusal = execute(&generated, slice, optimization);
        assert!(!refusal.status.success());
        assert!(String::from_utf8_lossy(&refusal.stderr)
            .contains("owned byte view exceeds the exact length bound"));
        let observed = execute(
            &generated,
            include_str!("owned_views_probe.c"),
            optimization,
        );
        assert!(
            observed.status.success(),
            "status {}; {}",
            observed.status,
            String::from_utf8_lossy(&observed.stderr)
        );
    }
}
