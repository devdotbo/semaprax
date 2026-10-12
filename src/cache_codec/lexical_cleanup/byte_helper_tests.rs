//! Selective v19 cache composition must preserve actual typed authority.
use crate::cleanup_plan::{CleanupPlan, CLEANUP_PLAN_SCHEMA_V19};

#[test]
fn helper_renewal_cache_roundtrips_with_and_without_lexical_control() {
    for lexical in [false, true] {
        // Acyclic final returns normalize to ordinary tails. A return inside
        // an admitted loop retains actual lexical authority.
        let tail = if lexical {
            "let mut index=0; while index<1 { index=index+1; if index==1 { return 7; }; 0 } 7"
        } else {
            "7"
        };
        let source = format!(
            r#"module cache.byte_helper;
@id("cache.fill") fn fill(output:own Bytes)->Bytes {{
 let mut next=output;next=bytes_set(next,0usize,65u8);next
}}
@id("cache.main") fn main()->i64 {{
 let mut output=bytes_zeroed(1usize);output=fill(output);{tail}
}}
"#
        );
        let ast = crate::check(&source, "helper-cache.spx").unwrap();
        let mut program = crate::hir::resolve(&ast).unwrap();
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "cache.main")
            .unwrap();
        assert_eq!(function.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V19);
        assert_eq!(function.cleanup_plan.lexical_control.is_some(), lexical);
        let encoded = super::super::encode(&function.cleanup_plan).unwrap();
        function.cleanup_plan = super::super::decode::<CleanupPlan>(&encoded).unwrap();
        assert_eq!(
            super::super::encode(&function.cleanup_plan).unwrap(),
            encoded
        );
        crate::hir::validate(&program).unwrap();
    }
}

#[test]
fn helper_cache_rejects_future_token_and_legacy_lexical_schema_mismatch() {
    let future = super::super::encode(&"semaprax.cleanup-plan.v21".to_owned()).unwrap();
    assert!(super::super::decode::<&'static str>(&future).is_err());
    let ast = crate::check(
        "module cache.legacy; @id(\"main\") fn main()->i64 {let mut index=0; while index<1 {index=index+1; if index==1 {return 7;}; 0} 7}",
        "legacy-cache.spx",
    )
    .unwrap();
    let program = crate::hir::resolve(&ast).unwrap();
    let mut plan = program.functions[0].cleanup_plan.clone();
    assert!(plan.lexical_control.is_some());
    plan.schema = crate::cleanup_plan::CLEANUP_PLAN_SCHEMA_V17;
    assert!(super::super::encode(&plan).is_err());
}
