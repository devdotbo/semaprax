//! Authored structured exits: exact advancement, suffix settlement and result staging.
use semaprax::interpreter;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);

const ITERATOR: &str = r#"module lexical.iterator;
@id("app.main") fn main()->i64 ensures result==22 {
 let one=vec_push<Bytes>(vec_with_capacity<Bytes>(3usize),bytes_zeroed(1usize));
 let two=vec_push<Bytes>(one,bytes_zeroed(2usize));
 let values=vec_push<Bytes>(two,bytes_zeroed(3usize));
 let mut seen=0; let mut total=0;
 for own item in vec_into_iter<Bytes>(values) {
  seen=seen+1;
  if seen==1 {continue;};
  if byte_len(bytes_as_slice(item))==2usize {total=total+2;} else {total=total+100;};
  if seen==2 {break;};
  0
 }
 return total+seen*10;
}
"#;

const RETURN_BYTES: &str = r#"module lexical.result;
@id("lexical.choose") fn choose(input:borrow Slice<u8>)->Bytes
 ensures byte_len(bytes_as_slice(result))==2usize {
 let outer=bytes_copy(input);
 if byte_len(input)==2usize {return outer;};
 return bytes_zeroed(3usize);
}
@id("app.main") fn main()->i64 {
 let input=[1u8,2u8];
 let value=choose(array_as_slice(input));
 if byte_len(bytes_as_slice(value))==2usize {29}else{0}
}
"#;

fn interpret(source: &str, expected: i64, failure: bool) {
    let path = std::env::temp_dir().join(format!(
        "lexical-owned-{}-{}.spx",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, source).unwrap();
    for _ in 0..3 {
        let observed = interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
        interpreter::verify_envelope(&observed.envelope).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&observed.envelope).unwrap();
        let outcome = &envelope["payload"]["outcome"];
        if failure {
            assert!(!observed.returned);
            assert_eq!(outcome["status"]["domain_id"], "semaprax.contract.v1");
            assert_eq!(outcome["status"]["code"], 2);
        } else {
            assert!(observed.returned);
            assert_eq!(outcome["value"], expected.to_string());
        }
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn owned_iterator_continue_advances_once_and_break_settles_suffix() {
    interpret(ITERATOR, 22, false);
    crate::owned_vec_bytes_runtime::run_wasm_expected(ITERATOR, 0, 3, "none", 22);
    let failed = ITERATOR.replace("return total+seen*10;", "return total+seen*10-1;");
    interpret(&failed, 0, true);
    crate::owned_vec_bytes_runtime::run_wasm_expected(&failed, 10, 3, "none", 0);
    interpret(ITERATOR, 22, false);
}

#[test]
fn returned_owned_result_stays_protected_until_selected_postconditions() {
    interpret(RETURN_BYTES, 29, false);
    crate::owned_vec_bytes_runtime::run_wasm_expected(RETURN_BYTES, 0, 1, "none", 29);
    let failed = RETURN_BYTES.replace("let input=[1u8,2u8];", "let input=[1u8];");
    interpret(&failed, 0, true);
    crate::owned_vec_bytes_runtime::run_wasm_expected(&failed, 10, 2, "none", 0);
    interpret(RETURN_BYTES, 29, false);
}
