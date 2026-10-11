use super::*;
#[test]
fn lexical_iterator_overlay_requires_retained_owner_and_independent_replay() {
    let source = r#"module lexical.graph.overlay;
@id("app.main") fn main()->i64 {
 let a=vec_push<Bytes>(vec_with_capacity<Bytes>(2usize),bytes_zeroed(1usize));
 let values=vec_push<Bytes>(a,bytes_zeroed(2usize));
 let mut seen=0;
 for own item in vec_into_iter<Bytes>(values) {
  seen=seen+1;
  if seen==1 {continue;} else {0}
  break;
 }
 return seen;
}"#;
    let checked = crate::check(source, "lexical-graph-overlay.spx").unwrap();
    let original = crate::hir::resolve(&checked).unwrap();
    crate::hir::validate(&original).unwrap();
    let function = original
        .functions
        .iter()
        .find(|f| f.id.as_str() == "app.main")
        .unwrap();
    assert_eq!(function.cleanup_plan.schema, CLEANUP_PLAN_SCHEMA_V18);
    assert!(iterator_loop_schema(
        Some(&original),
        None,
        [function],
        &original.function_templates
    )
    .is_ok());
    assert!(iterator_loop_schema(None, None, [function], &original.function_templates).is_err());
    for mutation in 0..2 {
        let mut program = original.clone();
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        if mutation == 0 {
            function.cleanup_plan.lexical_control = None;
        } else {
            function.cleanup_plan.schema = CLEANUP_PLAN_SCHEMA_V13;
        }
        let function = program
            .functions
            .iter()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        assert!(
            iterator_loop_schema(
                Some(&program),
                None,
                [function],
                &program.function_templates
            )
            .is_err(),
            "mutation {mutation}"
        );
    }
}
