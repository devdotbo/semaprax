//! Authored borrowed-row loops exercise existing read carriers on all engines.
use super::{run_wasm, DECLARATION, SEQUENCE};
use std::sync::atomic::Ordering;

fn run(body: &str, expected: i64) {
    let source = format!("{DECLARATION}{body}");
    let path = std::env::temp_dir().join(format!(
        "record-traversal-{}-{}.spx",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, &source).unwrap();
    let result =
        semaprax::interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
    std::fs::remove_file(path).unwrap();
    assert!(
        result
            .envelope
            .contains(&format!("\"value\":\"{expected}\"")),
        "{}",
        result.envelope
    );
    super::super::scoped_vec_field_native::run(&source, 0, expected);
    run_wasm(body, 0, expected, "none");
}

#[test]
fn owned_rows_preserve_ascending_indices_and_iteration_views_across_engines() {
    run(
        r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(3usize);
 let first=vec_push<Row>(empty,Row{priority:7,title:"é\u{0}",payload:bytes_zeroed(3usize)});
 let second=vec_push<Row>(first,Row{priority:11,title:"x",payload:bytes_zeroed(1usize)});
 let rows=vec_push<Row>(second,Row{priority:13,title:"",payload:bytes_zeroed(0usize)});
 let mut order=0; let mut good=true;
 for borrow row at index in rows {
  let text=row.title; let data=row.payload; let utf8=str_as_bytes(row.title);
  order=order*10+i64_from_usize(index)+1;
  good=good && byte_len(data)==byte_len(utf8) && str_len_bytes(text)==i64_from_usize(byte_len(utf8));
  let scalar=row.priority;
  good=good && scalar>0;
  0
 }
 let renewed=vec_sort_owned<Row>(rows);
 if good && vec_len<Row>(renewed)==3usize && order==123 {29}else{0}
}"#,
        29,
    );
}

#[test]
fn repeated_singleton_loops_expire_views_before_owner_reentry() {
    run(
        r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{priority:17,title:"é",payload:bytes_zeroed(2usize)});
 let mut observed=0;
 for borrow row in rows { let text=row.title; if str_len_bytes(text)==2 {observed=observed+row.priority;} 0 }
 for borrow again at position in rows { let data=again.payload; if position==0usize && byte_len(data)==2usize {observed=observed+again.priority;} 0 }
 let sorted=vec_sort_owned<Row>(rows);
 if observed==34 && vec_len<Row>(sorted)==1usize {29}else{0}
}"#,
        29,
    );
}

#[test]
fn malformed_host_row_refuses_before_scalar_or_view_publication() {
    run_wasm(
        r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(1usize);
 let rows=vec_push<Row>(empty,Row{priority:7,title:"x",payload:bytes_zeroed(1usize)});
 let mut seen=0;
 for borrow row in rows { let text=row.title; seen=seen+str_len_bytes(text); 0 }
 seen
}"#,
        0,
        1,
        "field-bad-word",
    );
}

// One scalar and two owned fields have a normative maximum of 4,096 rows.
// Construction precedes the first observed read; the reused native probe rejects
// every malloc/calloc/realloc afterward and checks complete settlement on reentry.
#[test]
fn maximum_admitted_owned_rows_preserve_views_order_and_nonallocating_reentry() {
    const SOURCE: &str = include_str!("../../fixtures/record-traversal-maximum.spx");
    let declaration = semaprax::format::canonical(
        &semaprax::parse(DECLARATION, "maximum-row-declaration.spx").unwrap(),
    );
    let body = SOURCE
        .strip_prefix(&declaration)
        .expect("exact canonical owning Row declaration, including stable fields and types");
    let expected: i64 = (0..4096).sum();
    assert_eq!(expected, 8_386_560);
    run(body, expected);
}
