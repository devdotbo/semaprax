use super::interpret;
const SOURCE: &str = r#"module lexical.finite.scratch;
@id("app.main") fn main()->i64 ensures result==4 {
 let a=vec_push<Bytes>(vec_with_capacity<Bytes>(2usize),bytes_zeroed(1usize));
 let values=vec_push<Bytes>(a,bytes_zeroed(2usize));
 let mut count=0;
 for own item in vec_into_iter<Bytes>(values) {
  let scratch=bytes_zeroed(3usize);
  count=count+2;
  if byte_len(bytes_as_slice(item))==1usize {continue;};
  if byte_len(bytes_as_slice(scratch))!=3usize {return 0;};
  0
 }
 return count;
}
"#;
#[test]
fn finite_iteration_scratch_preserves_cumulative_allocation_settlement_and_reentry() {
    interpret(SOURCE, 4, false);
    crate::owned_vec_bytes_runtime::run_wasm_expected(SOURCE, 0, 4, "none", 4);
    let failure = SOURCE.replace("return count;", "return count-1;");
    interpret(&failure, 0, true);
    crate::owned_vec_bytes_runtime::run_wasm_expected(&failure, 10, 4, "none", 0);
    interpret(SOURCE, 4, false);
}
