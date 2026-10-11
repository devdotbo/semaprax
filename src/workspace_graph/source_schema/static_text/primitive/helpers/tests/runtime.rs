//! Execute the imported closure and saved local control at the same targets.
use crate::hir::ResolvedProgram;
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
static SERIAL: AtomicU64 = AtomicU64::new(0);
pub(super) fn run(imported: &ResolvedProgram, local: &ResolvedProgram) {
    let expected = crate::interpreter::evaluate_resolved_zero_arg_i64(local, "app.main", 100_000)
        .unwrap()
        .outcome;
    let (failure, domain, code, value) = match &expected {
        crate::interpreter::ResolvedEvaluationOutcome::ReturnedI64(value) => (false, "", 0, *value),
        crate::interpreter::ResolvedEvaluationOutcome::LanguageFailure(status) => {
            (true, status.domain_id(), status.code(), 0)
        }
        _ => panic!("finite scalar control"),
    };
    let wire_code = match domain {
        "" => 0,
        "semaprax.arithmetic.v1" => code,
        "semaprax.contract.v1" => 8 + code,
        _ => panic!("unsupported scalar status domain"),
    };
    let root = std::env::temp_dir().join(format!(
        "semaprax-imported-scalar-{}-{}",
        std::process::id(),
        SERIAL.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    for (name, program) in [("imported", imported), ("local", local)] {
        let source = crate::codegen::emit_hir_c(program).unwrap();
        let probe = format!(
            r#"
int main(void){{
 struct spx_status_entry entries[32];struct spx_context context={{0}};
 if(!spx_context_init(&context,UINT64_C(17),entries,32,NULL,NULL,NULL))return 1;
 for(unsigned i=0;i<3;++i){{
  uint32_t before=context.status_arena.length;
  int64_t value=INT64_C(123456789);spx_status_token token=spx_decl_6170702e6d61696e(&context,&value);
  if({failure}){{
   if(token==SPX_STATUS_SUCCESS||value!=INT64_C(123456789))return 2;
   const struct spx_normalized_status*status=spx_status_resolve(&context,token);
   if(!status||strcmp(status->domain_id,"{domain}")||status->code!=UINT32_C({code}))return 3;
  }}else if(token!=SPX_STATUS_SUCCESS||value!=INT64_C({value}))return 4;
  if(context.call_depth||context.status_arena.length!=before+{failure})return 5;
 }}return 0;
}}
"#,
            failure = u8::from(failure)
        );
        let c = root.join(format!("{name}.c"));
        std::fs::write(&c, format!("{source}\n{probe}")).unwrap();
        for optimization in ["-O0", "-O2"] {
            let binary = root.join(format!("{name}-{optimization}"));
            let compiled = Command::new("clang")
                .args([
                    "-std=c11",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_NO_ENTRY_WRAPPER",
                    optimization,
                ])
                .arg(&c)
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(
                compiled.status.success(),
                "{}",
                String::from_utf8_lossy(&compiled.stderr)
            );
            let executed = Command::new(&binary).output().unwrap();
            assert!(
                executed.status.success(),
                "{name}: {:?}: {}",
                executed.status.code(),
                String::from_utf8_lossy(&executed.stderr)
            );
        }
        let wasm = root.join(format!("{name}.wasm"));
        std::fs::write(&wasm, crate::wasm::emit_resolved_module(program).unwrap()).unwrap();
        let host = root.join("host.js");
        std::fs::write(
            &host,
            include_str!("../../../../../../../tests/owned_data/owned_leaf_vec/host.js").replace(
                "if(!selected&&returned!==expectedValue)",
                "if(selected&&returned!==undefined)throw Error('failed helper published a result');\n    if(!selected&&returned!==expectedValue)",
            ),
        )
        .unwrap();
        let executed = Command::new("node")
            .arg(host)
            .arg(wasm)
            .arg(wire_code.to_string())
            .arg(value.to_string())
            .arg("prefix-failure")
            .output()
            .unwrap();
        assert!(
            executed.status.success(),
            "{name}: {} {}",
            String::from_utf8_lossy(&executed.stdout),
            String::from_utf8_lossy(&executed.stderr)
        );
    }
    std::fs::remove_dir_all(root).unwrap();
}
