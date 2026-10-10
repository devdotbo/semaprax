use super::runtime;

#[test]
fn copy_rows_optional_index_empty_and_singleton_execute_across_engines() {
    runtime::run(
        "borrowed-row-traversal",
        r#"module app.copy_traversal;
@id("row") record Row { @id("row.value") value:i64, @id("row.other") other:i64, }
@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(2usize);
 let mut observed=0;
 for borrow unused at zero in empty { observed=observed+unused.value+i64_from_usize(zero); 0 }
 let rows=vec_push<Row>(empty,Row{value:17,other:5});
 for borrow row at index in rows { observed=observed+row.value+row.other+i64_from_usize(index); 0 }
 for borrow again in rows { observed=observed+again.value+again.other; 0 }
 if observed==44 && vec_len<Row>(rows)==1usize {42}else{1}
}"#,
        0,
        42,
        false,
    );
}
