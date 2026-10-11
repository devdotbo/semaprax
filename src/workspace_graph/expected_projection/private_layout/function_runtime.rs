//! Owning executions of the same source-linked private signature layout.
use crate::hir::ResolvedProgram;
use std::process::Command;
use std::sync::atomic::{AtomicUsize, Ordering};
static NEXT: AtomicUsize = AtomicUsize::new(0);
pub(super) fn run(program: &ResolvedProgram, failure: bool) {
    let interpreted =
        crate::interpreter::evaluate_resolved_zero_arg_i64(program, "app.main", 100_000).unwrap();
    if failure {
        assert!(matches!(
            interpreted.outcome,
            crate::interpreter::ResolvedEvaluationOutcome::LanguageFailure(_)
        ));
    } else {
        assert!(matches!(
            interpreted.outcome,
            crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(42)
        ));
    }
    let root = std::env::temp_dir().join(format!(
        "semaprax-private-function-layout-{}-{}",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let generated = crate::codegen::emit_hir_c(program)
        .unwrap()
        .replace("malloc(", "layout_malloc(")
        .replace("calloc(", "layout_calloc(")
        .replace("realloc(", "layout_realloc(")
        .replace(
            "#define SPX_VEC_REALLOC realloc",
            "#define SPX_VEC_REALLOC layout_realloc",
        )
        .replace("free(", "layout_free(");
    let probe = include_str!("function_runtime.c")
        .replace("/* expected failure */", if failure { "1" } else { "0" });
    let allocator = r#"#include <stdint.h>
#include <stdlib.h>
#include <string.h>
static uint64_t layout_live;
static void*layout_malloc(size_t n){void*p=malloc(n);if(p)++layout_live;return p;}
static void*layout_calloc(size_t n,size_t s){void*p=calloc(n,s);if(p)++layout_live;return p;}
static __attribute__((unused)) void*layout_realloc(void*p,size_t n){void*r=realloc(p,n);if(r&&!p)++layout_live;return r;}
static void layout_free(void*p){if(p){if(!layout_live)abort();--layout_live;free(p);}}
"#;
    let c = root.join("layout.c");
    std::fs::write(&c, format!("{allocator}\n{generated}\n{probe}")).unwrap();
    for optimization in ["-O0", "-O2"] {
        let binary = root.join(format!("layout-{optimization}"));
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
                optimization,
            ])
            .arg(&c)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let executed = Command::new(&binary).output().unwrap();
        assert!(
            executed.status.success(),
            "{:?}: {}",
            executed.status.code(),
            String::from_utf8_lossy(&executed.stderr)
        );
    }
    let wasm = root.join("layout.wasm");
    std::fs::write(&wasm, crate::wasm::emit_resolved_module(program).unwrap()).unwrap();
    let host = root.join("host.js");
    std::fs::write(
        &host,
        include_str!("../../../../tests/owned_data/owned_leaf_vec/host.js"),
    )
    .unwrap();
    let executed = Command::new("node")
        .arg(host)
        .arg(wasm)
        .args(if failure {
            ["10", "0", "prefix-failure"]
        } else {
            ["0", "42", "prefix-failure"]
        })
        .output()
        .unwrap();
    assert!(
        executed.status.success(),
        "{}: {}",
        String::from_utf8_lossy(&executed.stdout),
        String::from_utf8_lossy(&executed.stderr)
    );
    std::fs::remove_dir_all(root).unwrap();
}
