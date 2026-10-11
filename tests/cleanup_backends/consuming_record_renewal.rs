//! Physical native settlement for the same consuming-row source, not a simulator.
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT: AtomicU64 = AtomicU64::new(0);
const SOURCE: &str = r#"module consuming.native;
@id("row") record Row {@id("row.n") n:i64,@id("row.text") text:string,@id("row.bytes") bytes:Bytes,}
@id("main") fn main()->i64 {
 let mut words=vec_with_capacity<Row>(2usize);
 let later=vec_with_capacity<Row>(1usize);
 let first=vec_push<Row>(vec_with_capacity<Row>(2usize),Row{n:7,text:"ab",bytes:bytes_zeroed(1usize)});
 let inputs=vec_push<Row>(first,Row{n:7,text:"ab",bytes:bytes_zeroed(1usize)});
 for own item in vec_into_iter<Row>(inputs) {words=vec_push<Row>(words,item);0}
 if vec_len<Row>(words)==2usize && vec_len<Row>(later)==0usize {29}else{0}
}
"#;
const OBSERVER: &str = r#"
#include <stdlib.h>
#include <stddef.h>
#include <string.h>
static void *live[1024];
static size_t owners=0,allocated=0,freed=0;
static int observer_error=0;
static void remember(void *p) { if(!p) return; if(owners==1024) {observer_error=1;return;} live[owners++]=p;allocated++; }
static void *observed_malloc(size_t n) {void *p=malloc(n);remember(p);return p;}
static void *observed_calloc(size_t n,size_t s) {void *p=calloc(n,s);remember(p);return p;}
static void observed_free(void *p) {
 if(!p) return;
 size_t i=0;while(i<owners&&live[i]!=p)i++;
 if(i==owners) {observer_error=1;return;}
 live[i]=live[--owners];freed++;free(p);
}
#define malloc observed_malloc
#define calloc observed_calloc
#define free observed_free
"#;
#[test]
fn consuming_row_staged_argument_failures_settle_once_preserve_status_and_reentry_o0_o2() {
    if Command::new("clang").arg("--version").output().is_err() {
        return;
    }
    let transformed = SOURCE
        .replace("@id(\"main\")", "@id(\"output\") record Output {@id(\"output.n\") n:i64,@id(\"output.text\") text:string,@id(\"output.bytes\") bytes:Bytes,}\n@id(\"main\")")
        .replace("let mut words=vec_with_capacity<Row>", "let mut words=vec_with_capacity<Output>")
        .replace("words=vec_push<Row>(words,item);0", "let _=match own item {Row{n,text,bytes}=>{if n==7 {words=vec_push<Output>(words,Output{n:n+1,text:text,bytes:bytes});}0},};0")
        .replace("if vec_len<Row>(words)==2usize", "let mut observed=0;for borrow row in words {let title=row.text;let data=row.bytes;if row.n==8 && str_len_bytes(title)==2 && byte_len(data)==1usize {observed=observed+1;}0} if observed==2 && vec_len<Output>(words)==2usize");
    let conditional_false = transformed
        .replace(
            "let inputs=vec_push<Row>(first,Row{n:7",
            "let inputs=vec_push<Row>(first,Row{n:8",
        )
        .replace(
            "observed==2 && vec_len<Output>(words)==2usize",
            "observed==1 && vec_len<Output>(words)==1usize",
        );
    for (source, code) in [
        (SOURCE.to_owned(), 0),
        (transformed.clone(), 0),
        (conditional_false, 0),
        (
            transformed.replace(
                "let mut words=vec_with_capacity<Output>(2usize)",
                "let mut words=vec_with_capacity<Output>(1usize)",
            ),
            1,
        ),
        (
            SOURCE.replace(
                "vec_push<Row>(words,item)",
                "vec_push<Row>(words,vec_clone_at<Row>(later,0usize))",
            ),
            2,
        ),
        (
            SOURCE.replace(
                "let mut words=vec_with_capacity<Row>(2usize)",
                "let mut words=vec_with_capacity<Row>(1usize)",
            ),
            1,
        ),
    ] {
        let program = semaprax::check(&source, "consuming-native.spx").unwrap();
        let generated = semaprax::codegen::emit_c(&program).unwrap();
        assert_eq!(generated, semaprax::codegen::emit_c(&program).unwrap());
        let symbol = format!(
            "spx_decl_{}",
            "main"
                .bytes()
                .map(|b| format!("{b:02x}"))
                .collect::<String>()
        );
        let probe = format!(
            r#"
int main(void) {{
 struct spx_status_entry entries[128];struct spx_context context={{0}};
 if(!spx_context_init(&context,744,entries,128,NULL,NULL,NULL))return 1;
 for(int pass=0;pass<4;pass++) {{
  int64_t output=INT64_C(1234567);size_t before_alloc=allocated,before_free=freed;
  spx_status_token token={symbol}(&context,&output);
  if({code}==0) {{if(token!=SPX_STATUS_SUCCESS||output!=29)return 2;}}
  else {{const struct spx_normalized_status *status=spx_status_resolve(&context,token);
   if(token==SPX_STATUS_SUCCESS||!status||strcmp(status->domain_id,"semaprax.vec.v1")!=0||status->code!={code}||output!=INT64_C(1234567))return 3;}}
  if(observer_error||owners!=0||allocated-before_alloc==0||allocated-before_alloc!=freed-before_free||context.call_depth!=0)return 4;
 }}return 0;
}}
"#
        );
        for optimization in ["-O0", "-O2"] {
            let id = NEXT.fetch_add(1, Ordering::Relaxed);
            let stem = format!("semaprax-consuming-row-{}-{id}", std::process::id());
            let c = std::env::temp_dir().join(format!("{stem}.c"));
            let executable =
                std::env::temp_dir().join(format!("{stem}{}", std::env::consts::EXE_SUFFIX));
            std::fs::write(&c, format!("{OBSERVER}\n{generated}\n{probe}")).unwrap();
            let compilation = Command::new("clang")
                .args([
                    "-std=c11",
                    optimization,
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_NO_ENTRY_WRAPPER",
                ])
                .arg(&c)
                .arg("-o")
                .arg(&executable)
                .output()
                .unwrap();
            assert!(
                compilation.status.success(),
                "{}",
                String::from_utf8_lossy(&compilation.stderr)
            );
            let execution = Command::new(&executable).output().unwrap();
            assert!(
                execution.status.success(),
                "{:?} {}",
                execution.status,
                String::from_utf8_lossy(&execution.stderr)
            );
            std::fs::remove_file(c).unwrap();
            std::fs::remove_file(executable).unwrap();
        }
    }
}
