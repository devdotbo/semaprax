//! Execute the actual checked scalar core and ordinary owned-String host route.
//! No new export/profile or substitute expression evaluator is introduced.
use semaprax::{ast::Program, check, codegen, wasm};
use std::path::Path;
use std::process::Command;

pub(in super::super) fn assert_execution_pair(
    authored: &str,
    ordinary: &str,
    expected: Option<i64>,
    internal_strings: bool,
) {
    assert!(
        super::super::command_available("clang"),
        "scalar statement backend evidence requires clang"
    );
    assert!(
        super::super::command_available("node"),
        "scalar statement backend evidence requires Node"
    );
    for (label, text) in [("authored", authored), ("ordinary", ordinary)] {
        let source = super::super::write_source(label, text);
        let root = source.parent().unwrap();
        let program = check(text, super::PATH).unwrap();
        native(&program, root, expected);
        core_wasm(&program, root, expected, internal_strings);
        std::fs::remove_dir_all(root).unwrap();
    }
}

pub(in super::super) fn native(program: &Program, root: &Path, expected: Option<i64>) {
    let generated = codegen::emit_c(program).unwrap();
    let expected_check = match expected {
        Some(value) => format!("if(status!=SPX_STATUS_SUCCESS || value!=INT64_C({value})) return 2;"),
        None => r#"if(status==SPX_STATUS_SUCCESS || value!=INT64_C(0x2525252525252525)) return 3;
        const struct spx_normalized_status *entry=spx_status_resolve(&context,status);
        if(entry==NULL || strcmp(entry->domain_id,"semaprax.arithmetic.v1")!=0 || entry->code!=UINT32_C(4)) return 4;
        if(context.status_arena.length!=before+UINT32_C(1)) return 5;"#.to_owned(),
    };
    let before = if expected.is_none() {
        "uint32_t before=context.status_arena.length;"
    } else {
        ""
    };
    let probe = format!(
        r#"
int main(void) {{
    struct spx_status_entry entries[UINT32_C(16)]; struct spx_context context={{0}};
    if(!spx_context_init(&context,UINT64_C(91),entries,UINT32_C(16),NULL,NULL,NULL)) return 1;
    for(unsigned run=0;run<3;run++) {{
        {before}
        int64_t value=INT64_C(0x2525252525252525);
        spx_status_token status=spx_decl_6d61696e(&context,&value);
        {expected_check}
    }}
    return 0;
}}
"#
    );
    for optimization in ["-O0", "-O2"] {
        let source = root.join(format!("probe{optimization}.c"));
        let binary = root.join(format!(
            "probe{optimization}{}",
            std::env::consts::EXE_SUFFIX
        ));
        std::fs::write(&source, format!("{generated}\n{probe}")).unwrap();
        let output = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{optimization}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let output = Command::new(binary).output().unwrap();
        assert!(output.status.success(), "{optimization}: {output:?}");
        assert!(output.stdout.is_empty());
        assert!(output.stderr.is_empty());
    }
}

fn core_wasm(program: &Program, root: &Path, expected: Option<i64>, internal_strings: bool) {
    if internal_strings {
        selected_strings(program, root, expected);
        return;
    }
    let web = root.join("web");
    wasm::build_web(program, &web).unwrap();
    // The package carries exactly the ordinary Core Wasm emitter's module; its
    // generated runtime is the host whose checked statuses are asserted below.
    assert_eq!(
        std::fs::read(web.join("app.wasm")).unwrap(),
        wasm::emit_module(program).unwrap()
    );
    std::fs::write(web.join("package.json"), "{\"type\":\"module\"}\n").unwrap();
    let expected_check=match expected {
        Some(value)=>format!("if(instance.exports.semaprax_main()!=={value}n) throw Error('scalar statement value');"),
        None=>r#"let failed=false;
        try { instance.exports.semaprax_main(); } catch(error) {
            const status=semanticStatus(error);
            if(status===null || status.domain_id!=="semaprax.arithmetic.v1" || status.code!==4) throw error;
            failed=true;
        }
        if(!failed) throw Error('discarded call published its later tail');"#.to_owned(),
    };
    std::fs::write(
        web.join("probe.mjs"),
        format!(
            r#"
import {{readFile}} from 'node:fs/promises';
import {{instantiateBytes,semanticStatus}} from './semaprax.js';
const {{instance}}=await instantiateBytes(await readFile('./app.wasm'));
for(let run=0;run<3;run++) {{ {expected_check} }}
"#
        ),
    )
    .unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(web)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "Core-Wasm: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}

// The legacy scalar Web package intentionally lacks String imports. Exercise
// its refusal separately, then select the existing authenticated String arena
// with exactly one i64 export. No String/owning value crosses that host API.
fn selected_strings(program: &Program, root: &Path, expected: Option<i64>) {
    assert_eq!(
        wasm::build_web(program, &root.join("legacy-refused"))
            .unwrap_err()
            .code,
        "SPX-W116"
    );
    assert!(!root.join("legacy-refused").exists());
    let artifact = wasm::internal_strings::emit_module(
        program,
        &["main".to_owned()],
        wasm::internal_strings::InternalStringOptions::default(),
    )
    .unwrap();
    let web = root.join("internal-strings");
    std::fs::create_dir(&web).unwrap();
    std::fs::write(web.join("app.wasm"), artifact.wasm_bytes()).unwrap();
    std::fs::write(web.join("descriptor.json"), artifact.descriptor()).unwrap();
    std::fs::write(web.join("runtime.mjs"), artifact.runtime_source()).unwrap();
    let expected_check=match expected {
        Some(value)=>format!("if(outcome.kind!=='success' || outcome.value!=={value}n) throw Error('discarded String call result');"),
        None=>r#"if(outcome.kind!=='failure' || outcome.domain!=="semaprax.arithmetic.v1" || outcome.code!==4 || Object.hasOwn(outcome,'value')) throw Error('discarded String call failure');"#.to_owned(),
    };
    std::fs::write(
        web.join("probe.mjs"),
        format!(
            r#"
import {{readFile}} from 'node:fs/promises';
import {{webcrypto}} from 'node:crypto';
if(globalThis.crypto===undefined) Object.defineProperty(globalThis,'crypto',{{value:webcrypto}});
import {{instantiate}} from './runtime.mjs';
const bytes=new Uint8Array(await readFile('./app.wasm'));
const runtime=await instantiate(bytes);
for(let run=0;run<3;run++) {{
    const outcome=runtime.call('main');
    {expected_check}
}}
const forged=bytes.slice(); forged[0]^=1;
let rejected=false;
try {{ await instantiate(forged); }} catch(error) {{
    if(!/authentication/.test(String(error))) throw error;
    rejected=true;
}}
if(!rejected) throw Error('unbound provider bytes admitted');
"#
        ),
    )
    .unwrap();
    let output = Command::new("node")
        .arg("probe.mjs")
        .current_dir(web)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "selected Core-Wasm: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stdout.is_empty());
    assert!(output.stderr.is_empty());
}
