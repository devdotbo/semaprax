//! Independently observed same-cell consuming row transfer; original fixture is retained.
use std::sync::atomic::Ordering;
const BODY: &str = r#"@id("app.main") fn main()->i64 {
 let mut words=vec_with_capacity<Row>(2usize);
 let later=vec_with_capacity<Row>(1usize);
 let first=vec_push<Row>(vec_with_capacity<Row>(2usize),Row{priority:7,title:"ab",payload:bytes_zeroed(1usize)});
 let inputs=vec_push<Row>(first,Row{priority:7,title:"ab",payload:bytes_zeroed(1usize)});
 for own item in vec_into_iter<Row>(inputs) {words=vec_push<Row>(words,item);0}
 if vec_len<Row>(words)==2usize && vec_len<Row>(later)==0usize {29}else{0}
}
"#;
fn interpret(body: &str, value: i64, code: Option<u32>) {
    let path = std::env::temp_dir().join(format!(
        "consuming-row-renewal-{}-{}.spx",
        std::process::id(),
        super::SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, format!("{}{}", super::DECLARATION, body)).unwrap();
    let execution =
        semaprax::interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
    semaprax::interpreter::verify_envelope(&execution.envelope).unwrap();
    let document: serde_json::Value = serde_json::from_str(&execution.envelope).unwrap();
    let outcome = &document["payload"]["outcome"];
    if let Some(code) = code {
        assert!(!execution.returned);
        assert_eq!(outcome["status"]["domain_id"], "semaprax.vec.v1");
        assert_eq!(outcome["status"]["code"], code);
    } else {
        assert!(execution.returned);
        assert_eq!(outcome["value"], value.to_string());
    }
    std::fs::remove_file(path).unwrap();
}
#[test]
fn consuming_rows_keep_later_cell_and_settle_partial_push_failure_on_reentry() {
    // run_wasm's unchanged host requires zero live Vec/Iterator/String/Bytes
    // owners and equal physical payload-copy/drop counts after every call.
    for _ in 0..3 {
        interpret(BODY, 29, None);
        super::run_wasm(BODY, 0, 29, "none");
    }
    let failed = BODY.replace(
        "vec_push<Row>(words,item)",
        "vec_push<Row>(words,vec_clone_at<Row>(later,0usize))",
    );
    // The original failing second argument must retain the staged outer Vec,
    // unmoved yielded item and remainder cleanup, with selected Vec index status.
    interpret(&failed, 0, Some(2));
    super::run_wasm(&failed, 14, 0, "none");
    let full = BODY.replace(
        "let mut words=vec_with_capacity<Row>(2usize)",
        "let mut words=vec_with_capacity<Row>(1usize)",
    );
    interpret(&full, 0, Some(1));
    super::run_wasm(&full, 13, 0, "none");
    interpret(BODY, 29, None);
    super::run_wasm(BODY, 0, 29, "none");
}
