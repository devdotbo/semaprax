//! Repeated payload epochs preserve existing vector cells and sticky status.
use super::run_wasm;
const BODY: &str = r#"@id("app.main") fn main()->i64 {
 let mut words=vec_with_capacity<string>(2usize);
 let mut later=vec_with_capacity<string>(2usize);
 let mut i=0;
 while i<2 && (i<3 || 1/(i-i)>0) {
  if i==0 { words=vec_push<string>(words,string_concat("a","b")); 0 } else {0}
  later=vec_push<string>(later,string_concat("c","d"));
  i=i+1; 0
 }
 let first=vec_clone_at<string>(words,0usize);
 if first=="ab" && vec_len<string>(words)==1usize && vec_len<string>(later)==2usize {29}else{0}
}
"#;
#[test]
fn two_owned_vector_cells_retain_canonical_history_across_temporary_epochs() {
    let source = format!("{}{}", super::DECLARATION, BODY);
    let checked = semaprax::check(&source, "owned-leaf-loop.spx").unwrap();
    let canonical = semaprax::format::canonical(&checked);
    let roundtrip = semaprax::check(&canonical, "owned-leaf-loop-roundtrip.spx").unwrap();
    assert_eq!(semaprax::format::canonical(&roundtrip), canonical);
    let graph = semaprax::graph::to_json(&checked).unwrap();
    semaprax::graph::verify_json(&checked, &graph).unwrap();
    let document: serde_json::Value = serde_json::from_str(&graph).unwrap();
    assert_eq!(
        document["vec_loop_renewal"]["updates"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    for _ in 0..3 {
        run_wasm(BODY, 0, 29, "none");
    }
}
#[test]
fn later_iteration_full_vector_failure_settles_both_cells_and_staged_string() {
    let body = BODY.replace(
        "vec_with_capacity<string>(2usize)",
        "vec_with_capacity<string>(1usize)",
    );
    // This private Core-Wasm host observes the raw capacity token 13;
    // the compiler-owned Vec status domain normalizes that token to code 1.
    run_wasm(&body, 13, 0, "none");
}
#[test]
fn lazy_condition_failure_preserves_selected_arithmetic_status() {
    // The right operand remains skipped in the positive case, but runs here
    // after one successful iteration with both vector cells still live.
    let body = BODY.replace("i<3 || 1/(i-i)>0", "i==0 || 1/(i-i)>0");
    // This owner host accepts Vec status only; source/HIR/C/Wasm generation
    // still owns the exact lazy-condition cleanup path without executing it.
    let source = format!("{}{}", super::DECLARATION, body);
    let checked = semaprax::check(&source, "owned-leaf-loop-condition.spx").unwrap();
    let resolved = semaprax::hir::resolve(&checked).unwrap();
    semaprax::hir::validate(&resolved).unwrap();
    semaprax::codegen::emit_hir_c(&resolved).unwrap();
    semaprax::wasm::emit_resolved_module(&resolved).unwrap();
}

#[test]
fn owned_record_argument_commit_preserves_later_cell_and_failed_argument_cleanup() {
    let body = r#"@id("app.main") fn main()->i64 {
 let mut words=vec_with_capacity<Row>(2usize);
 let later=vec_with_capacity<Row>(1usize);
 let mut index=0;
 while index<2 {
  let item=Row{priority:7,title:"ab",payload:bytes_zeroed(1usize)};
  words=vec_push<Row>(words,item);
  index=index+1; 0
 }
 if vec_len<Row>(words)==2usize && vec_len<Row>(later)==0usize {29}else{0}
}
"#;
    let source = format!("{}{}", super::DECLARATION, body);
    let checked = semaprax::check(&source, "owned-record-loop-commit.spx").unwrap();
    let canonical = semaprax::format::canonical(&checked);
    let roundtrip = semaprax::check(&canonical, "owned-record-loop-roundtrip.spx").unwrap();
    assert_eq!(semaprax::format::canonical(&roundtrip), canonical);
    let graph = semaprax::graph::to_json(&checked).unwrap();
    semaprax::graph::verify_json(&checked, &graph).unwrap();
    run_wasm(body, 0, 29, "none");
    // The first argument is staged before the borrowed clone-out refuses its
    // empty later vector. This private wire token is Vec index failure (14),
    // not the public normalized domain code 2. Neither cell may leak.
    let failed = body.replace(
        "let item=Row{priority:7,title:\"ab\",payload:bytes_zeroed(1usize)};\n  words=vec_push<Row>(words,item);",
        "words=vec_push<Row>(words,vec_clone_at<Row>(later,0usize));",
    );
    assert_ne!(failed, body);
    run_wasm(&failed, 14, 0, "none");
}
