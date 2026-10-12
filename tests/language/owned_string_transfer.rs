//! Explicit transfer plus unchanged clone semantics on actual owned engines.
use super::owned_string_loops_v1::support::Fixture;
use semaprax::interpreter::InterpreterOptions;
use semaprax::{codegen, hir, interpreter, wasm};
use std::process::Command;

const SOURCE: &str = r#"module transfer.runtime;
@id("take") fn take(text:string)->string {string_take(text)}
@id("copy") fn copy(text:string)->string {text}
@id("sink") fn sink(text:string,n:i64)->i64 {string_len(text)+n}
@id("reject") fn reject(text:string)->i64 requires string_len(text)<0 {string_len(text)}
@id("badpost") fn badpost(text:string)->string ensures string_len(result)<0 {string_take(text)}
@id("unicode") fn unicode()->i64 {string_len(take("é\u{0}"))}
@id("empty") fn empty()->i64 {string_len(take(""))}
@id("clone") fn clone()->i64 {string_len(copy("é\u{0}"))}
@id("late") fn late()->i64 {let text="é\u{0}";sink(string_take(text),1/0)}
@id("pre") fn pre()->i64 {let text="é\u{0}";reject(string_take(text))}
@id("post") fn post()->i64 {let text="é\u{0}";string_len(badpost(string_take(text)))}
@id("main") fn main()->i64 {unicode()}
"#;
fn symbol(id: &str) -> String {
    format!(
        "spx_decl_{}",
        id.bytes().map(|b| format!("{b:02x}")).collect::<String>()
    )
}

#[test]
fn explicit_transfer_preserves_native_carriers_clone_baseline_and_failure_settlement() {
    let ast = semaprax::check(SOURCE, "string-transfer-runtime.spx").unwrap();
    let program = hir::resolve(&ast).unwrap();
    hir::validate(&program).unwrap();
    let generated = codegen::emit_c(&ast).unwrap();
    assert_eq!(generated, codegen::emit_c(&ast).unwrap());
    // The allocation observer encloses original String construction through final
    // settlement. Fixed context/observer metadata are initialized before each call.
    // Existing realloc requests are counted too, without changing settlement hooks.
    let tracked = generated.replace("realloc(", "transfer_realloc(");
    let mut probe=format!("{}\n{}\nstatic unsigned transfer_realloc_requests;\nstatic __attribute__((unused)) void *transfer_realloc(void *p,size_t n){{++transfer_realloc_requests;return realloc(p,n);}}\n{tracked}\n#undef malloc\n#undef free\nint main(void){{struct spx_status_entry entries[32];struct spx_context context={{0}};REQUIRE(spx_context_init(&context,19,entries,32,NULL,NULL,NULL));\n",
 include_str!("../support/native_fixture_stdio.c"),include_str!("../native_owned_utf8_settlement_v1/allocations.c"));
    for (id, value, allocations) in [("unicode", 3, 1), ("empty", 0, 1), ("clone", 3, 2)] {
        probe.push_str(&format!("for(unsigned i=0;i<3;++i){{uint64_t before=fixture_allocations;unsigned requests=transfer_realloc_requests;int64_t result=INT64_MIN;REQUIRE({}(&context,&result)==0);REQUIRE(result=={value});REQUIRE(fixture_allocations-before=={allocations});REQUIRE(transfer_realloc_requests==requests);REQUIRE(fixture_live==0&&fixture_allocations==fixture_frees);}}\n",symbol(id)));
    }
    for (id, domain, code) in [
        ("late", "semaprax.arithmetic.v1", 4),
        ("pre", "semaprax.contract.v1", 1),
        ("post", "semaprax.contract.v1", 2),
    ] {
        probe.push_str(&format!("for(unsigned i=0;i<3;++i){{int64_t result=INT64_MIN;spx_status_token token={}(&context,&result);REQUIRE(token!=0);const struct spx_normalized_status *status=spx_status_resolve(&context,token);REQUIRE(status!=NULL);REQUIRE(strcmp(status->domain_id,\"{domain}\")==0&&status->code=={code});REQUIRE(result==INT64_MIN);REQUIRE(fixture_live==0&&fixture_allocations==fixture_frees);}}\n",symbol(id)));
    }
    probe.push_str("return 0;}\n");
    let mut fixture = Fixture::new(SOURCE);
    for level in ["-O0", "-O2"] {
        assert_eq!(fixture.native(&probe, level), "");
    }
    fixture.cleanup();
}

#[test]
fn explicit_transfer_interpreter_and_private_wasm_reentry_keep_selected_status() {
    let ast = semaprax::check(SOURCE, "string-transfer-runtime.spx").unwrap();
    let mut fixture = Fixture::new(SOURCE);
    for _ in 0..3 {
        for (id, value) in [("unicode", 3), ("empty", 0), ("clone", 3)] {
            let report = interpreter::internal_strings::interpret(
                &fixture.source,
                id,
                &[],
                &InterpreterOptions::default(),
            )
            .unwrap();
            assert!(report.returned);
            assert!(
                report.envelope.contains(&format!("\"value\":\"{value}\"")),
                "{}",
                report.envelope
            );
            interpreter::internal_strings::verify_envelope_against_source(
                &report.envelope,
                &fixture.source,
            )
            .unwrap();
        }
        for (id, domain, code) in [
            ("late", "semaprax.arithmetic.v1", 4),
            ("pre", "semaprax.contract.v1", 1),
            ("post", "semaprax.contract.v1", 2),
        ] {
            let report = interpreter::internal_strings::interpret(
                &fixture.source,
                id,
                &[],
                &InterpreterOptions::default(),
            )
            .unwrap();
            assert!(!report.returned);
            let envelope: serde_json::Value = serde_json::from_str(&report.envelope).unwrap();
            let outcome = &envelope["payload"]["outcome"];
            assert_eq!(outcome["kind"], "failed");
            assert_eq!(outcome["status"]["domain_id"], domain);
            assert_eq!(outcome["status"]["code"], code);
            assert!(outcome.get("value").is_none());
            interpreter::internal_strings::verify_envelope_against_source(
                &report.envelope,
                &fixture.source,
            )
            .unwrap();
        }
    }
    let exports = ["unicode", "empty", "clone", "late", "pre", "post"].map(str::to_owned);
    let artifact = wasm::internal_strings::emit_module(
        &ast,
        &exports,
        wasm::internal_strings::InternalStringOptions::default(),
    )
    .unwrap();
    fixture.write("program.wasm", artifact.wasm_bytes());
    fixture.write("program.mjs", artifact.runtime_source());
    let script=fixture.write("probe.mjs",r#"import {readFileSync} from 'node:fs';
import {instantiate} from './program.mjs';
const api=await instantiate(Uint8Array.from(readFileSync('program.wasm')));
for(let i=0;i<3;i++){
 for(const [id,value] of [['unicode',3n],['empty',0n],['clone',3n]]){const r=api.call(id);if(r.kind!=='success'||r.value!==value)throw Error(JSON.stringify(r));}
 for(const [id,domain,code] of [['late','semaprax.arithmetic.v1',4],['pre','semaprax.contract.v1',1],['post','semaprax.contract.v1',2]]){const r=api.call(id);if(r.kind!=='failure'||r.domain!==domain||r.code!==code||'value'in r)throw Error(JSON.stringify(r));}
}
"#);
    let output = Command::new("node")
        .current_dir(&fixture.root)
        .arg(script)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    fixture.cleanup();
}
