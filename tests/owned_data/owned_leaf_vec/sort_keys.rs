use super::*;
#[test]
fn explicit_owned_rankings_keep_stable_utf8_prefix_nul_ties_and_carriers() {
    let source = include_str!("../../fixtures/explicit-owned-sort-keys.spx");
    let body = source
        .strip_prefix("module owned.leaf;\n")
        .unwrap()
        .replace("owned.leaf.main", "app.main");
    run_wasm(&body, 0, 29, "none");
    let path = std::env::temp_dir().join(format!(
        "owned-ranking-{}-{}.spx",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).unwrap();
    let actual = semaprax::interpreter::interpret(
        &path,
        "owned.leaf.main",
        &[],
        &semaprax::interpreter::InterpreterOptions::default(),
    )
    .unwrap();
    std::fs::remove_file(path).unwrap();
    semaprax::interpreter::verify_envelope(&actual.envelope).unwrap();
    let envelope: serde_json::Value = serde_json::from_str(&actual.envelope).unwrap();
    assert!(actual.returned);
    assert_eq!(envelope["payload"]["outcome"]["value"], "29");
}
#[test]
fn explicit_sort_cannot_move_a_generation_with_a_live_field_view() {
    let body = r#"@id("app.main") fn main()->i64 {
 let a=vec_with_capacity<Row>(1usize);let b=vec_push<Row>(a,Row{priority:0,title:"x",payload:bytes_zeroed(0usize)});
 let view=vec_field<Row>(b,0usize,"title");let c=vec_sort_by<Row>(b,"priority:asc");
 if str_len_bytes(view)==1 {29}else{0}
}"#;
    assert!(semaprax::check(&format!("{DECLARATION}{body}"), "live-sort-view.spx").is_err());
}

#[test]
fn explicit_bytes_keys_use_unsigned_prefix_order_without_payload_cloning() {
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let a=[0u8,255u8];let b=[0u8];let c=[255u8];
 let mut v=vec_with_capacity<Row>(3usize);
 v=vec_push<Row>(v,Row{priority:0,title:"same",payload:bytes_copy(array_as_slice(a))});
 v=vec_push<Row>(v,Row{priority:1,title:"same",payload:bytes_copy(array_as_slice(b))});
 v=vec_push<Row>(v,Row{priority:2,title:"same",payload:bytes_copy(array_as_slice(c))});
 v=vec_sort_by<Row>(v,"payload:asc");
 let ascending=vec_field<Row>(v,0usize,"priority")==1 && vec_field<Row>(v,1usize,"priority")==0 && vec_field<Row>(v,2usize,"priority")==2;
 v=vec_sort_by<Row>(v,"payload:desc");
 if ascending && vec_field<Row>(v,0usize,"priority")==2 && vec_field<Row>(v,1usize,"priority")==0 && vec_field<Row>(v,2usize,"priority")==1 {29}else{0}
}"#,
        0,
        29,
        "none",
    );
}
