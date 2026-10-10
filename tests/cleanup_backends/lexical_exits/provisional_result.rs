//! Physical Bytes result remains callee-owned until borrowed ensures settle.
use super::{Command, Ordering, Path, NEXT};

const SOURCE: &str = r#"module test.provisional_native;
@id("probe") fn probe(fail:bool)->Bytes
 ensures byte_len(bytes_as_slice(result))==2usize {
 if fail {return bytes_zeroed(3usize);};
 return bytes_zeroed(2usize);
}
@id("main") fn main()->i64 {0}
"#;
#[test]
fn borrowed_ensures_preserves_poisoned_out_zero_leaks_and_reentry_o0_o2() {
    let ast = semaprax::check(SOURCE, Path::new("provisional-native.spx")).unwrap();
    let generated = semaprax::codegen::emit_c(&ast).unwrap();
    assert_eq!(generated, semaprax::codegen::emit_c(&ast).unwrap());
    let observer = r#"
#include <stdlib.h>
#include <stddef.h>
#include <string.h>
static size_t allocated=0, freed=0;
static void *observed_malloc(size_t n) {void *p=malloc(n);if(p!=NULL)allocated++;return p;}
static void *observed_calloc(size_t n,size_t width) {void *p=calloc(n,width);if(p!=NULL)allocated++;return p;}
static void observed_free(void *p) {if(p!=NULL)freed++;free(p);}
#define malloc observed_malloc
#define calloc observed_calloc
#define free observed_free
"#;
    let probe = r#"
int main(void) {
 struct spx_status_entry entries[16];struct spx_context context={0};
 if(!spx_context_init(&context,742,entries,16,NULL,NULL,NULL))return 10;
 uint8_t poison=211;
 for(int pass=0;pass<4;pass++) {
  spx_bytes_v1 result={.ptr=&poison,.len=UINT64_C(991)};
  size_t a=allocated,f=freed;
  spx_status_token status=spx_decl_70726f6265(&context,pass%2!=0,&result);
  if(pass%2!=0) {
   const struct spx_normalized_status *selected=spx_status_resolve(&context,status);
   if(status==SPX_STATUS_SUCCESS||selected==NULL||strcmp(selected->domain_id,"semaprax.contract.v1")!=0||selected->code!=2)return 11;
   if(result.ptr!=&poison||result.len!=UINT64_C(991)||poison!=211)return 12;
   if(allocated-a!=1||freed-f!=1)return 13;
  } else {
   if(status!=SPX_STATUS_SUCCESS||result.len!=UINT64_C(2)||result.ptr==&poison||result.ptr==NULL)return 14;
   if(result.ptr[0]!=0||result.ptr[1]!=0||allocated-a!=1||freed-f!=0)return 15;
   spx_bytes_drop(&result);
   if(result.ptr!=NULL||result.len!=0||allocated-a!=1||freed-f!=1)return 16;
  }
  if(context.call_depth!=0||allocated!=freed)return 17;
 }
 return 0;
}
"#;
    for optimization in ["-O0", "-O2"] {
        let unique = NEXT.fetch_add(1, Ordering::Relaxed);
        let stem = format!(
            "semaprax-provisional-native-{}-{unique}",
            std::process::id()
        );
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
            "{optimization}: {}",
            String::from_utf8_lossy(&compiled.stderr)
        );
        let run = Command::new(&binary).output().unwrap();
        std::fs::remove_file(source).unwrap();
        std::fs::remove_file(binary).unwrap();
        assert!(run.status.success(), "{optimization}: {run:?}");
    }
}
