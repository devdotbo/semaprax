use super::super::{native_runtime, COutput};
pub(in crate::codegen) const STRING_RUNTIME: &str = include_str!("runtime.c");
#[allow(clippy::too_many_arguments)] // The existing independently selected native carrier switches.
pub(in crate::codegen) fn emit_status_runtime(
    output: &mut impl COutput,
    vec_fields: bool,
    borrowed_str: bool,
    vec_authority: bool,
    box_authority: bool,
    owned_iterator: bool,
    immutable_list: bool,
) {
    let mut runtime = String::new();
    native_runtime::emit_status_runtime_for_profile(
        &mut runtime,
        vec_fields,
        borrowed_str,
        vec_authority,
        box_authority,
        owned_iterator,
        immutable_list,
    );
    let anchor = "struct spx_context {";
    assert_eq!(runtime.matches(anchor).count(), 1);
    runtime=runtime.replacen(anchor,"struct spx_final_utf8_lease;\nstruct spx_context {\n    struct spx_final_utf8_lease *final_utf8_lease;",1);
    let anchor = "context->trace_generation == UINT64_C(0) &&";
    assert_eq!(runtime.matches(anchor).count(), 1);
    runtime = runtime.replacen(
        anchor,
        "context->trace_generation == UINT64_C(0) &&\n        context->final_utf8_lease == NULL &&",
        1,
    );
    output.push_str(&runtime);
}
