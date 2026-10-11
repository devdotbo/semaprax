//! #758: independent ordinary-call control and actual checked allocator refusal observations.
use super::{wasm, DECLARATION, SEQUENCE};
use std::{path::Path, process::Command, sync::atomic::Ordering};

fn source(constructor: bool) -> String {
    let construction = if constructor {
        "let rows=vec_of<Row>(e0(),e1(),e2(),e3());"
    } else {
        // Independently authored control: never derive this from the constructor expansion.
        "let mut rows=vec_with_capacity<Row>(4usize);\n\
         rows=vec_push<Row>(rows,e0());\n\
         rows=vec_push<Row>(rows,e1());\n\
         rows=vec_push<Row>(rows,e2());\n\
         rows=vec_push<Row>(rows,e3());"
    };
    format!(
        r#"{DECLARATION}
@id("e0") fn e0()->Row {{let title="zero";let payload=bytes_zeroed(3usize);let checkpoint=vec_with_capacity<Bytes>(1usize);Row{{priority:0,title:title,payload:payload}}}}
@id("e1") fn e1()->Row {{let title="one";let payload=bytes_zeroed(3usize);let checkpoint=vec_with_capacity<Bytes>(1usize);Row{{priority:1,title:title,payload:payload}}}}
@id("e2") fn e2()->Row {{let title="two";let payload=bytes_zeroed(3usize);let checkpoint=vec_with_capacity<Bytes>(1usize);Row{{priority:2,title:title,payload:payload}}}}
@id("e3") fn e3()->Row {{let title="three";let payload=bytes_zeroed(3usize);let checkpoint=vec_with_capacity<Bytes>(1usize);Row{{priority:3,title:title,payload:payload}}}}
@id("app.main") fn main()->i64 {{
 {construction}
 let mut good=true; let mut seen=0usize;
 for borrow row at index in rows {{
  let text=row.title; let data=row.payload;
  good=good && row.priority==i64_from_usize(index) && str_len_bytes(text)>0 && byte_len(data)==3usize;
  seen=seen+1usize; 0
 }}
 if good && seen==4usize && vec_len<Row>(rows)==4usize {{42}}else{{-1}}
}}"#
    )
}

// Track identities as well as totals: a double finalizer cannot cancel a leak.
const ALLOCATOR: &str = r#"#include <stdint.h>
#include <stdlib.h>
#include <stdio.h>
#include <string.h>
static void *owners[64];
static unsigned attempts,fail_at,acquired,settled,trace,expected_trace[65];
static int observe;
static int refuse(void){
 ++attempts;
 if(attempts>64)abort();
 if(observe)expected_trace[attempts]=trace;
 return attempts==fail_at;
}
static void track(void *p){
 if(!p)return;
 for(unsigned i=0;i<64;++i)if(!owners[i]){owners[i]=p;++acquired;return;}
 abort();
}
static unsigned owner_slot(void *p){
 for(unsigned i=0;i<64;++i)if(owners[i]==p)return i;
 abort();
}
static void *probe_malloc(size_t n){void*p=malloc(n);track(p);return p;}
static void *probe_calloc(size_t n,size_t s){void*p=calloc(n,s);track(p);return p;}
static void *probe_checked_calloc(size_t n,size_t s){if(refuse())return NULL;return probe_calloc(n,s);}
static __attribute__((unused)) void *probe_realloc(void*p,size_t n){
 unsigned slot=p?owner_slot(p):0;
 void*r=realloc(p,n);
 if(r){if(p)owners[slot]=r;else track(r);}return r;
}
static void probe_free(void*p){if(p){owners[owner_slot(p)]=NULL;++settled;free(p);}}
static void element(unsigned n){
 /* Decimal trace records both multiplicity and order, including the first item. */
 trace=trace*10+n;
 if(trace>1234)abort();
}
"#;

const PROBE: &str = r#"
static void reset(unsigned failure){attempts=0;fail_at=failure;acquired=0;settled=0;trace=0;}
static int clean(struct spx_context *context){
 if(acquired!=settled||context->call_depth)return 0;
 for(unsigned i=0;i<64;++i)if(owners[i])return 0;
 for(uint32_t i=0;i<SPX_VEC_AUTHORITY_CAPACITY;++i)if(context->vec_authority[i].live)return 0;
 return 1;
}
int main(void){
 struct spx_status_entry entries[256];struct spx_context context={0};
 if(!spx_context_init(&context,17,entries,256,NULL,NULL,NULL))return 1;
 reset(0);observe=1;int64_t value=INT64_C(123456789);
 spx_status_token token=spx_decl_6170702e6d61696e(&context,&value);
 observe=0;
 if(token!=SPX_STATUS_SUCCESS||value!=42||trace!=1234||!clean(&context))return 2;
 unsigned total=attempts;
 if(total!=5||expected_trace[1]!=0||expected_trace[2]!=1||
    expected_trace[3]!=12||expected_trace[4]!=123||expected_trace[5]!=1234)return 3;
 int middle=0;
 for(unsigned ordinal=1;ordinal<=total;++ordinal){
  if(expected_trace[ordinal]==12||expected_trace[ordinal]==123)middle=1;
  for(unsigned repeat=0;repeat<3;++repeat){
   unsigned before=context.status_arena.length;
   reset(ordinal);value=INT64_C(123456789);
   token=spx_decl_6170702e6d61696e(&context,&value);
   if(token==SPX_STATUS_SUCCESS||value!=INT64_C(123456789)||attempts!=ordinal)return 4;
   if(trace!=expected_trace[ordinal]||!clean(&context))return 5;
   const struct spx_normalized_status *status=spx_status_resolve(&context,token);
   if(!status||strcmp(status->domain_id,"semaprax.vec.v1")||status->code!=3||context.status_arena.length!=before+1)return 6;
   printf("%u %u %llu %s %u %u %u %u\n",ordinal,repeat,(unsigned long long)token,
          status->domain_id,status->code,trace,acquired,settled);
   /* Reentry immediately after every physical refusal, with the same context. */
   reset(0);value=INT64_C(123456789);
   spx_status_token next=spx_decl_6170702e6d61696e(&context,&value);
   if(next!=SPX_STATUS_SUCCESS||value!=42||trace!=1234||attempts!=total||!clean(&context))return 7;
   if(spx_status_resolve(&context,token)!=status||context.status_arena.length!=before+1)return 8;
  }
 }
 if(!middle)return 9;
 return 0;
}
"#;

fn native(generated: String, root: &Path, label: &str) -> Vec<Vec<u8>> {
    let mut generated = generated;
    for (symbol, marker) in [
        ("spx_decl_6530", 1),
        ("spx_decl_6531", 2),
        ("spx_decl_6532", 3),
        ("spx_decl_6533", 4),
    ] {
        // Find the actual function body, skipping declarations and call sites.
        let positions: Vec<_> = generated
            .match_indices(symbol)
            .filter_map(|(at, _)| {
                let tail = &generated[at + symbol.len()..];
                let close = tail.find(')')?;
                let rest = &tail[close + 1..];
                rest.trim_start()
                    .starts_with('{')
                    .then(|| at + symbol.len() + close + 1 + rest.find('{').unwrap() + 1)
            })
            .collect();
        assert_eq!(positions.len(), 1, "exact emitted definition for {symbol}");
        generated.insert_str(positions[0], &format!("\n element({marker});\n"));
    }
    assert_eq!(
        generated.matches("storage = calloc(1, bytes)").count(),
        1,
        "exact owned-leaf capacity allocation site"
    );
    let generated = generated
        .replace("malloc(", "probe_malloc(")
        .replace("calloc(", "probe_calloc(")
        .replace("realloc(", "probe_realloc(")
        .replace(
            "#define SPX_VEC_REALLOC realloc",
            "#define SPX_VEC_REALLOC probe_realloc",
        )
        .replace("free(", "probe_free(")
        .replace(
            "storage = probe_calloc(1, bytes)",
            "storage = probe_checked_calloc(1, bytes)",
        )
        .replace(
            "(spx_bytes_v1 *)probe_calloc(",
            "(spx_bytes_v1 *)probe_checked_calloc(",
        )
        .replace(
            "(spx_vec_record_v1 *)probe_calloc(",
            "(spx_vec_record_v1 *)probe_checked_calloc(",
        );
    let path = root.join(format!("{label}.c"));
    std::fs::write(&path, format!("{ALLOCATOR}\n{generated}\n{PROBE}")).unwrap();
    ["-O0", "-O2"]
        .into_iter()
        .map(|opt| {
            let binary = root.join(format!("{label}-{opt}"));
            let built = Command::new("clang")
                .args([
                    "-std=c11",
                    "-Wall",
                    "-Wextra",
                    "-Werror",
                    "-DSPX_NO_ENTRY_WRAPPER",
                    opt,
                ])
                .arg(&path)
                .arg("-o")
                .arg(&binary)
                .output()
                .unwrap();
            assert!(
                built.status.success(),
                "{label} {opt}: {}",
                String::from_utf8_lossy(&built.stderr)
            );
            let output = Command::new(binary).output().unwrap();
            assert!(
                output.status.success(),
                "{label} {opt}: {:?}: {}",
                output.status.code(),
                String::from_utf8_lossy(&output.stderr)
            );
            output.stdout
        })
        .collect()
}

#[test]
fn constructor_matches_explicit_chain_at_every_actual_native_allocation_and_wasm_commit() {
    let root = std::env::temp_dir().join(format!(
        "vec-constructor-fault-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let mut observations = Vec::new();
    for (label, constructor) in [("constructor", true), ("explicit", false)] {
        let source = source(constructor);
        let ast = semaprax::check(&source, root.join(format!("{label}.spx"))).unwrap();
        observations.push(native(
            semaprax::codegen::emit_c(&ast).unwrap(),
            &root,
            label,
        ));
        // Element-local checked capacities are actual allocation sites; legacy String/Bytes
        // allocation failures remain invariant failures and are counted, never refused here.
        // Existing strict private host asserts all payload/vector finalizers and reentry.
        for (status, mode) in [
            (0, "none"),
            (15, "allocation"),
            (15, "codec-push-1"),
            (15, "codec-push-2"),
            (15, "codec-push-4"),
        ] {
            wasm(&ast, &root, status, mode);
        }
    }
    assert_eq!(
        observations[0], observations[1],
        "status token/domain/code, markers and exact owner settlements"
    );
    assert_eq!(
        observations[0][0], observations[0][1],
        "O0/O2 physical refusal observations"
    );
    std::fs::remove_dir_all(root).unwrap();
}
