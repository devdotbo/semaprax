//! Real allocation/drop observations for source-authenticated fresh contract locals.
//! Contract refusal is recoverable; native physical allocation failure still has
//! the existing fatal constructor contract. This is not String OOM recovery.
use std::path::Path;
use std::process::Command;
use std::sync::atomic::Ordering;

const SOURCE: &str = r#"module contracts.native;
@id("contract.inspect") fn inspect(text:string)->bool {string_len(text)==2}
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

#[test]
fn fresh_contract_owners_settle_once_before_publication_and_preserve_sticky_refusal_o0_o2() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let program = semaprax::check(SOURCE, Path::new("contract-owners-native.spx")).unwrap();
    let canonical = semaprax::format::canonical(&program);
    assert_eq!(
        canonical,
        semaprax::format::canonical(
            &semaprax::check(&canonical, "contract-owners-native.spx").unwrap()
        )
    );
    let generated = semaprax::codegen::emit_c(&program).unwrap();
    assert_eq!(generated, semaprax::codegen::emit_c(&program).unwrap());
    let symbol = format!(
        "spx_decl_{}",
        "contract.run"
            .bytes()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let observer = r#"
#include <stdlib.h>
#include <stddef.h>
#include <string.h>
#include <stdio.h>
static size_t allocated=0, freed=0;
static void *observed_malloc(size_t n) {
    void *p=malloc(n); if(p!=NULL) allocated+=1; return p;
}
static void *observed_calloc(size_t n,size_t size) {
    void *p=calloc(n,size); if(p!=NULL) allocated+=1; return p;
}
static void observed_free(void *p) { if(p!=NULL) freed+=1; free(p); }
#define malloc observed_malloc
#define calloc observed_calloc
#define free observed_free
"#;
    let probe = format!(
        r#"
int main(void) {{
 struct spx_status_entry entries[64]; struct spx_context context={{0}};
 if(!spx_context_init(&context,743,entries,64,NULL,NULL,NULL)) return 1;
 for(int pass=0;pass<4;pass+=1) {{
  int64_t output=INT64_C(1234567);
  size_t before_alloc=allocated,before_free=freed;
  spx_status_token token={symbol}(&context,pass==1,pass==2,&output);
  if(pass==1||pass==2) {{
   const struct spx_normalized_status *status=spx_status_resolve(&context,token);
   if(token==SPX_STATUS_SUCCESS||status==NULL||strcmp(status->domain_id,"semaprax.contract.v1")!=0||status->code!=(pass==1?UINT32_C(1):UINT32_C(2))||output!=INT64_C(1234567)) return 2;
  }} else if(token!=SPX_STATUS_SUCCESS||output!=INT64_C(29)) return 3;
  size_t expected=pass==1?1:4;
  if(allocated-before_alloc!=expected||freed-before_free!=expected||allocated!=freed||context.call_depth!=0) {{
   fprintf(stderr,"contract-owner observation pass=%d allocated=%zu freed=%zu expected=%zu total_allocated=%zu total_freed=%zu depth=%zu\n",pass,allocated-before_alloc,freed-before_free,expected,allocated,freed,(size_t)context.call_depth);
   return 4;
  }}
 }}
 return 0;
}}
"#
    );
    for optimization in ["-O0", "-O2"] {
        let unique = super::NEXT.fetch_add(1, Ordering::Relaxed);
        let stem = format!("semaprax-contract-owners-{}-{unique}", std::process::id());
        let source = std::env::temp_dir().join(format!("{stem}.c"));
        let binary = std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
        std::fs::write(&source, format!("{observer}\n{generated}\n{probe}")).unwrap();
        let compiled = Command::new("clang")
            .args([
                "-std=c11",
                optimization,
                "-Wall",
                "-Wextra",
                "-Werror",
                "-DSPX_NO_ENTRY_WRAPPER",
            ])
            .arg(&source)
            .arg("-o")
            .arg(&binary)
            .output()
            .unwrap();
        assert!(
            compiled.status.success(),
            "{}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let observed = Command::new(&binary).output().unwrap();
        std::fs::remove_file(source).unwrap();
        std::fs::remove_file(binary).unwrap();
        assert!(observed.status.success(), "{optimization}: {observed:?}");
    }
}
