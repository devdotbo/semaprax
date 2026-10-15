//! Independent business rankings preserve the original declaration/wire layout.
use super::*;
#[test]
fn explicit_three_rankings_are_stable_and_identical_across_all_engines() {
    runtime::run(
        "explicit-rankings",
        include_str!("../../fixtures/explicit-copy-sort-keys.spx"),
        0,
        42,
        false,
    );
}
#[test]
fn explicit_keys_cover_empty_singleton_full_and_signed_unsigned_boundaries() {
    let decl = "module bounded; @id(\"bounded.row\") record R { @id(\"bounded.serial\") serial:i64, @id(\"bounded.key\") key:i64, @id(\"bounded.unsigned\") unsigned:usize, }";
    for (name,body) in [
        ("empty", "let v=vec_with_capacity<R>(0usize);let w=vec_sort_by<R>(v,\"key:desc\");if vec_len<R>(w)==0usize {42}else{1}"),
        ("singleton", "let v=vec_with_capacity<R>(1usize);let w=vec_push<R>(v,R{serial:7,key:-9223372036854775808,unsigned:18446744073709551615usize});let x=vec_sort_by<R>(w,\"key:desc,unsigned:asc\");let r=vec_get<R>(x,0usize);if r.serial==7 && r.key==-9223372036854775808 && r.unsigned==18446744073709551615usize {42}else{1}"),
        ("full", "let mut v=vec_with_capacity<R>(2730usize);let mut i=0usize;while i<2730usize {v=vec_push<R>(v,R{serial:i64_from_usize(i),key:0,unsigned:18446744073709551615usize});i=i+1usize;0}v=vec_sort_by<R>(v,\"key:desc,unsigned:asc\");let first=vec_get<R>(v,0usize);let last=vec_get<R>(v,2729usize);if first.serial==0 && last.serial==2729 && vec_len<R>(v)==2730usize && vec_capacity<R>(v)==2730usize {42}else{1}"),
    ] { runtime::run(name,&format!("{decl} @id(\"app.main\") fn main()->i64 {{{body}}}"),0,42,false); }
}

#[test]
fn explicit_float_keys_preserve_ieee_signed_zero_total_order() {
    let source = r#"module floating;
@id("float.row") record F { @id("float.serial") serial:i64,@id("float.key") key:f64, }
@id("app.main") fn main()->i64 {
 let mut v=vec_with_capacity<F>(4usize);
 v=vec_push<F>(v,F{serial:0,key:-0.0});v=vec_push<F>(v,F{serial:1,key:0.0});
 v=vec_push<F>(v,F{serial:2,key:-1.5});v=vec_push<F>(v,F{serial:3,key:2.5});
 v=vec_sort_by<F>(v,"key:asc");
 let a=vec_get<F>(v,0usize);let b=vec_get<F>(v,1usize);let c=vec_get<F>(v,2usize);let d=vec_get<F>(v,3usize);
 let ascending=a.serial==2 && b.serial==0 && c.serial==1 && d.serial==3;
 v=vec_sort_by<F>(v,"key:desc");
 let e=vec_get<F>(v,0usize);let f=vec_get<F>(v,1usize);let g=vec_get<F>(v,2usize);let h=vec_get<F>(v,3usize);
 if ascending && e.serial==3 && f.serial==1 && g.serial==0 && h.serial==2 {42}else{1}
}"#;
    runtime::run("explicit-floating-f64", source, 0, 42, false);
    let f32 = source
        .replace(":f64", ":f32")
        .replace("-0.0", "-0.0f32")
        .replace("key:0.0", "key:0.0f32")
        .replace("-1.5", "-1.5f32")
        .replace("2.5", "2.5f32");
    runtime::run("explicit-floating-f32", &f32, 0, 42, false);
}
