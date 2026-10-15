//! New authoring syntax does not expand the frozen public closure profile.
use super::*;
#[test]
fn constructors_in_selected_private_helpers_refuse_but_unselected_helpers_do_not_expand_closure() {
    let source = r#"module public.constructor;
@id("helper") fn helper()->i64 {let values=vec_of<i64>(1,2);i64_from_usize(vec_len<i64>(values))}
@id("api") fn api()->i64 {42}
@id("app.main") fn main()->i64 {0}
"#;
    let program = resolve(source);
    let descriptor =
        derive_public_api_descriptor(&program, &["api".to_owned()], subject()).unwrap();
    replay_public_api_descriptor(
        &program,
        &["api".to_owned()],
        subject(),
        &descriptor.canonical_bytes(),
        &descriptor.digest(),
    )
    .unwrap();
    let selected = resolve(&source.replace("fn api()->i64 {42}", "fn api()->i64 {helper()}"));
    let error =
        derive_public_api_descriptor(&selected, &["api".to_owned()], subject()).unwrap_err();
    assert_eq!(error.code, "SPX-J113");
    assert!(
        error
            .message
            .contains("Vec constructors are outside the frozen profile"),
        "{error:?}"
    );
}
