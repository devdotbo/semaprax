//! Borrowed-row views expire at each selected break/continue before reentry.
use super::{run_wasm, DECLARATION, SEQUENCE};
use std::sync::atomic::Ordering;
#[test]
fn row_views_expire_before_continue_latch_and_break_owner_reentry() {
    let body = r#"@id("app.main") fn main()->i64 ensures result==29 {
  let first=vec_push<Row>(vec_with_capacity<Row>(2usize),Row{priority:3,title:"é",payload:bytes_zeroed(1usize)});
  let rows=vec_push<Row>(first,Row{priority:7,title:"x",payload:bytes_zeroed(2usize)});
  let mut seen=0;let mut selected=0;
  for borrow row at index in rows {
   let text=row.title;let data=row.payload;
   seen=seen+1;
   if index==0usize && str_len_bytes(text)==2 {continue;};
   if byte_len(data)==2usize {selected=row.priority;};
   break;
  }
  let reordered=vec_sort_owned<Row>(rows);
  if seen==2 && selected==7 && vec_len<Row>(reordered)==2usize {return 29;};
  return 0;
 }"#;
    let source = format!("{DECLARATION}{body}");
    let path = std::env::temp_dir().join(format!(
        "lexical-row-{}-{}.spx",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, &source).unwrap();
    let observed =
        semaprax::interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
    let envelope: serde_json::Value = serde_json::from_str(&observed.envelope).unwrap();
    assert!(observed.returned);
    assert_eq!(envelope["payload"]["outcome"]["value"], "29");
    run_wasm(body, 0, 29, "none");
    run_wasm(body, 0, 29, "none");
    std::fs::remove_file(path).unwrap();
}
