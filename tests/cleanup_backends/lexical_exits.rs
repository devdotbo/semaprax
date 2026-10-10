//! Actual native exits: shared postconditions, output poisoning, and owned cleanup.
use std::path::Path;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = r#"
module test.lexical_exits;
@id("lexical.run")
fn run(fail: bool) -> i64 ensures result == 9 {
    let outer = "outer";
    let mut index = 0;
    while index < 5 {
        let inner = "inner";
        index = index + 1;
        if index == 2 { continue; };
        if index == 4 { break; };
    }
    if fail { return 8; };
    return if index == 4 { 9 } else { 0 };
}
@id("app.main")
fn main() -> i64 { run(false) }
"#;

#[test]
fn native_lexical_loops_return_cleanup_and_failed_ensures_preserve_reentry_o0_o2() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let parsed = semaprax::parse(SOURCE, Path::new("native-lexical-exits.spx")).unwrap();
    let generated = semaprax::codegen::emit_c(&parsed).unwrap();
    assert_eq!(generated, semaprax::codegen::emit_c(&parsed).unwrap());
    assert!(generated.contains("spx_lexical_latch_"));
    assert!(generated.contains("spx_lexical_break_"));
    assert!(generated.contains("goto spx_postconditions;"));
    let symbol = format!(
        "spx_decl_{}",
        "lexical.run"
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let observer = r#"
#include <stdlib.h>
#include <stddef.h>
#include <string.h>
static size_t spx_test_allocated = 0;
static size_t spx_test_freed = 0;
static void *spx_test_malloc(size_t size) {
    void *value = malloc(size);
    if (value != NULL) spx_test_allocated += 1;
    return value;
}
static void spx_test_free(void *value) {
    if (value != NULL) spx_test_freed += 1;
    free(value);
}
#define malloc spx_test_malloc
#define free spx_test_free
"#;
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[32];
    struct spx_context context = {{0}};
    if (!spx_context_init(&context, 734, entries, 32, NULL, NULL, NULL)) return 10;
    for (int pass = 0; pass < 3; pass += 1) {{
        int64_t result = INT64_C(1234567);
        size_t allocated_before = spx_test_allocated;
        size_t freed_before = spx_test_freed;
        spx_status_token status = {symbol}(&context, pass == 1, &result);
        if (pass == 1) {{
            const struct spx_normalized_status *selected = spx_status_resolve(&context, status);
            if (status == SPX_STATUS_SUCCESS || selected == NULL || strcmp(selected->domain_id, "semaprax.contract.v1") != 0 || selected->code != UINT32_C(2) || result != INT64_C(1234567)) return 11;
        }} else if (status != SPX_STATUS_SUCCESS || result != INT64_C(9)) return 12;
        if (spx_test_allocated - allocated_before != 5 || spx_test_freed - freed_before != 5) return 13;
    }}
    return 0;
}}
"#
    );
    for optimization in ["-O0", "-O2"] {
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-lexical-native-{}-{unique}", std::process::id());
        let source = std::env::temp_dir().join(format!("{stem}.c"));
        let binary = std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source, format!("{observer}\n{generated}\n{probe}")).unwrap();
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
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let run = Command::new(&binary).output().unwrap();
        std::fs::remove_file(&source).unwrap();
        std::fs::remove_file(&binary).unwrap();
        assert!(run.status.success(), "{optimization}: {run:?}");
    }
}
