//! Same admitted source on interpreter and the strict existing Core-Wasm host.
use std::sync::atomic::Ordering;
const SOURCE: &str = r#"@id("contract.inspect") fn inspect(text:string)->bool {string_len(text)==2}
@id("contract.run") fn run(fail_pre:bool,fail_post:bool)->i64
 requires {let required=bytes_zeroed(2usize);let moved=required;byte_len(bytes_as_slice(moved))==2usize && !fail_pre}
 ensures {let checked=bytes_zeroed(3usize);let text="ok";let moved=text;inspect(moved) && byte_len(bytes_as_slice(checked))==3usize && !fail_post}
 ensures result==29 {
 let body="body";
 let mut index=0;
 while index<1 {index=index+1;if index==1 {return string_len(body)+25;};0}
 29
}
@id("app.main") fn main()->i64 {run(false,false)}
"#;
fn interpret(source: &str, value: i64, code: Option<u32>) {
    let path = std::env::temp_dir().join(format!(
        "contract-owners-{}-{}.spx",
        std::process::id(),
        super::SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::write(&path, format!("{}{}", super::DECLARATION, source)).unwrap();
    for _ in 0..3 {
        let observed =
            semaprax::interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
        semaprax::interpreter::verify_envelope(&observed.envelope).unwrap();
        let envelope: serde_json::Value = serde_json::from_str(&observed.envelope).unwrap();
        let outcome = &envelope["payload"]["outcome"];
        if let Some(code) = code {
            assert!(!observed.returned);
            assert_eq!(outcome["status"]["domain_id"], "semaprax.contract.v1");
            assert_eq!(outcome["status"]["code"], code);
        } else {
            assert!(observed.returned);
            assert_eq!(outcome["value"], value.to_string());
        }
    }
    std::fs::remove_file(path).unwrap();
}
#[test]
fn fresh_contract_aliases_consume_locals_settle_on_refusal_and_reenter_without_owners() {
    // The existing host verifies empty vectors/iterators/payloads and equal
    // physical copy/drop counts on every invocation, including failed calls.
    interpret(SOURCE, 29, None);
    super::run_wasm(SOURCE, 0, 29, "none");
    for (call, code) in [("run(true,false)", 1), ("run(false,true)", 2)] {
        let failed = SOURCE.replace("run(false,false)", call);
        interpret(&failed, 0, Some(code));
        super::run_wasm(&failed, 10, 0, "none");
    }
    interpret(SOURCE, 29, None);
    super::run_wasm(SOURCE, 0, 29, "none");
}
