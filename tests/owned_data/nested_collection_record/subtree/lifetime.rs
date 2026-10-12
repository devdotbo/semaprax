//! Runtime boundaries for live transferred children, independent of metadata.
use super::*;

#[test]
fn live_subtree_break_and_continue_settle_on_every_engine() {
    let text = format!(
        r#"{PREFIX}
@id("app.main") fn main()->i64 {{
 let mut count=0;
 while count<3 {{
  let outer=Outer{{valid:true,report:Report{{bytes:bytes_zeroed(3usize),text:"four",words:vec_with_capacity<string>(0usize),metrics:Metrics{{total:35}}}},sibling:Sibling{{bytes:bytes_zeroed(1usize),tag:9}}}};
  match own outer {{Outer{{sibling:Sibling{{bytes:unused,tag:_}},report,valid:_}}=>{{
   count=count+1;
   if count==1 {{continue;}}else{{if count==2 {{break;}}else{{score(report)}}}}
  }},}};
  0
 }}
 if count==2 {{42}}else{{0}}
}}
"#
    );
    let root = directory("subtree-live-exits");
    let path = root.join("app.spx");
    std::fs::write(&path, &text).unwrap();
    let ast = semaprax::check(&text, &path).unwrap();
    let hir = semaprax::hir::resolve(&ast).unwrap();
    semaprax::hir::validate(&hir).unwrap();
    let main = hir
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    let control = main
        .cleanup_plan
        .lexical_control
        .as_ref()
        .expect("actual lexical proof");
    for kind in [
        semaprax::ast::LexicalExitKind::Break,
        semaprax::ast::LexicalExitKind::Continue,
    ] {
        assert!(
            control.exits.iter().any(|exit| exit.kind == kind),
            "real {kind:?} exit"
        );
    }
    for _ in 0..3 {
        let execution =
            interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
        assert!(execution.returned, "{}", execution.envelope);
        interpreter::verify_envelope(&execution.envelope).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&execution.envelope).unwrap();
        assert_eq!(envelope["payload"]["outcome"]["value"], "42");
    }
    // Existing probes repeat invocations and require every heap owner freed,
    // native call depth/status arena restored, and all Vec grants retired.
    super::super::super::owned_collection_outcome::run_native(&ast, &root);
    super::super::super::owned_collection_outcome::run_strict_wasm(&ast, &root);
    std::fs::remove_dir_all(root).unwrap();
}

fn insert_entry_probe(generated: &str, symbol: &str, probe: &str) -> String {
    let mut matches = 0;
    let mut output = String::new();
    for line in generated.lines() {
        output.push_str(line);
        output.push('\n');
        if line.contains(&format!("spx_status_token {symbol}(")) && line.ends_with(" {") {
            matches += 1;
            output.push_str(probe);
            output.push('\n');
        }
    }
    assert_eq!(matches, 1, "exact current function definition for {symbol}");
    output
}

#[test]
fn subtree_forwarding_allocates_no_replacement_tree_or_payload_copy_at_runtime() {
    // A default owned String read clones. Keep the original recursive source
    // as an exact copied control; the explicit transfer is the independent
    // recursive zero-copy baseline for whole-subtree movement.
    assert_eq!(RECURSIVE.matches("text:text").count(), 1);
    let transferred = RECURSIVE.replace("text:text", "text:string_take(text)");
    for populated in [false, true] {
        for (label, pattern, clones) in [
            ("subtree", SUBTREE, 0u8),
            ("recursive", transferred.as_str(), 0u8),
            ("original-recursive-clone", RECURSIVE, 1u8),
        ] {
            let root = directory(&format!("{label}-no-copy"));
            let text = source(pattern, populated);
            let ast = semaprax::check(&text, root.join("app.spx")).unwrap();
            let generated = semaprax::codegen::emit_c(&ast).unwrap();
            // Actual allocator census begins at forward entry, after literal
            // owners exist; score entry is the first consumer of the child.
            // Count memcpy/memmove work and every realloc attempt (including existing
            // backing, allocator-internal copying and refusals) independently. Ordinary
            // inline aggregate metadata assignments are not payload copies.
            let generated = generated
                .replace("memcpy(", "subtree_memcpy(")
                .replace("memmove(", "subtree_memmove(")
                .replace("realloc(", "subtree_resize(")
                .replace(
                    "#define SPX_VEC_REALLOC realloc",
                    "#define SPX_VEC_REALLOC subtree_resize",
                );
            let generated=insert_entry_probe(&generated,"spx_decl_666f7277617264",
                "    ++subtree_forward_entries; subtree_before_allocations=outcome_total_allocations; subtree_before_copies=subtree_payload_copies; subtree_before_resize=subtree_resize_requests; subtree_before_copy_bytes=subtree_payload_copy_bytes;");
            let generated=insert_entry_probe(&generated,"spx_decl_73636f7265",
                &format!("    ++subtree_score_entries; if(outcome_total_allocations-subtree_before_allocations!={clones} || subtree_payload_copies-subtree_before_copies!={clones} || subtree_resize_requests!=subtree_before_resize || subtree_payload_copy_bytes-subtree_before_copy_bytes!={})abort();", 4 * u64::from(clones)));
            let probes = r#"#include <string.h>
static uint64_t subtree_before_allocations=0,subtree_before_copies=0,subtree_payload_copies=0;
static uint64_t subtree_forward_entries=0,subtree_score_entries=0;
static uint64_t subtree_before_copy_bytes=0,subtree_payload_copy_bytes=0;
static uint64_t subtree_before_resize=0,subtree_resize_requests=0;
static __attribute__((unused)) void *subtree_resize(void *p,size_t n){++subtree_resize_requests;return realloc(p,n);}
static __attribute__((destructor)) void subtree_probe_done(void){if(subtree_forward_entries!=3||subtree_score_entries!=3)abort();}
static __attribute__((unused)) void *subtree_memcpy(void *to,const void *from,size_t n){if(n){++subtree_payload_copies;subtree_payload_copy_bytes+=n;}return memcpy(to,from,n);}
static __attribute__((unused)) void *subtree_memmove(void *to,const void *from,size_t n){if(n){++subtree_payload_copies;subtree_payload_copy_bytes+=n;}return memmove(to,from,n);}
"#;
            // Uses actual O0/O2 runtime allocator and canonical settlement
            // observer, three fresh invocations each. No compiler allocations
            // or fixed tracking metadata are described as runtime payloads.
            super::super::super::owned_collection_outcome::run_native_generated(
                format!("{probes}{generated}"),
                &root,
            );
            std::fs::remove_dir_all(root).unwrap();
        }
    }
}

#[test]
fn finite_record_named_slice_copy_preserves_live_exits_on_every_engine() {
    let text = format!(
        r#"{PREFIX}
@id("app.main") fn main()->i64 {{
 let input=[0u8;3];let view=array_as_slice(input);let mut count=0;
 while count<3 {{
  let outer=Outer{{valid:true,report:Report{{bytes:bytes_copy(view),text:"four",words:vec_with_capacity<string>(0usize),metrics:Metrics{{total:35}}}},sibling:Sibling{{bytes:bytes_zeroed(1usize),tag:9}}}};
  match own outer {{Outer{{sibling:Sibling{{bytes:unused,tag:_}},report,valid:_}}=>{{
   count=count+1;
   if count==1 {{continue;}}else{{if count==2 {{break;}}else{{score(report)}}}}
  }},}};
  0
 }}
 if count==2 {{42}}else{{0}}
}}
"#
    );
    let root = directory("subtree-finite-named-copy");
    let path = root.join("app.spx");
    std::fs::write(&path, &text).unwrap();
    let ast = semaprax::check(&text, &path).unwrap();
    let hir = semaprax::hir::resolve(&ast).unwrap();
    semaprax::hir::validate(&hir).unwrap();
    let graph = semaprax::graph::to_json(&ast).unwrap();
    semaprax::graph::verify_json(&ast, &graph).unwrap();
    let wire: serde_json::Value = serde_json::from_str(&graph).unwrap();
    let main = wire["portable_indexed_byte_data"]["capacity_summaries"]
        .as_array().unwrap().iter().find(|summary| summary["function"] == "app.main").unwrap();
    assert_eq!(main["bytes_copy_sites"], 6);
    assert_eq!(main["owned_byte_payload_bytes"], 12);
    for _ in 0..3 {
        let execution = interpreter::interpret(&path, "app.main", &[], &InterpreterOptions::default()).unwrap();
        assert!(execution.returned, "{}", execution.envelope);
        interpreter::verify_envelope(&execution.envelope).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&execution.envelope).unwrap();
        assert_eq!(envelope["payload"]["outcome"]["value"], "42");
    }
    super::super::super::owned_collection_outcome::run_native(&ast, &root);
    super::super::super::owned_collection_outcome::run_strict_wasm(&ast, &root);
    std::fs::remove_dir_all(root).unwrap();
}
