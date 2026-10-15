#[test]
fn finite_trace_profile_refuses_lexical_cleanup_without_changing_legacy_admission() {
    use crate::cleanup_plan::*;
    assert!(!super::trace_path_schema_is_admitted(
        CLEANUP_PLAN_SCHEMA_V18
    ));
    for schema in [
        CLEANUP_PLAN_SCHEMA_V2,
        CLEANUP_PLAN_SCHEMA_V3,
        CLEANUP_PLAN_SCHEMA_V4,
        CLEANUP_PLAN_SCHEMA_V5,
        CLEANUP_PLAN_SCHEMA_V14,
    ] {
        assert!(super::trace_path_schema_is_admitted(schema));
    }
}
