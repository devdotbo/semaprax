//! Distinct resumes share one authored Next carrier, never one emission domain.
use super::interpret;
const SOURCE: &str = r#"module lexical.iterator.closed;
@id("app.main") fn main()->i64 ensures result==33 {
 let a=vec_push<Bytes>(vec_with_capacity<Bytes>(4usize),bytes_zeroed(1usize));
 let b=vec_push<Bytes>(a,bytes_zeroed(2usize));
 let c=vec_push<Bytes>(b,bytes_zeroed(3usize));
 let values=vec_push<Bytes>(c,bytes_zeroed(4usize));
 let mut seen=0;let mut total=0;
 for own item in vec_into_iter<Bytes>(values) {
  let width=byte_len(bytes_as_slice(item));
  seen=seen+1;
  if seen==1 {continue;} else {0}
  if width==2usize {continue;} else {0}
  total=total+3;
  break;
 }
 return total+seen*10;
}"#;
#[test]
fn two_resume_sites_and_closed_normal_latch_preserve_graph_cleanup_and_reentry() {
    let checked = semaprax::check(SOURCE, "closed-iterator.spx").unwrap();
    let canonical = semaprax::format::canonical(&checked);
    let roundtrip = semaprax::check(&canonical, "closed-iterator.spx").unwrap();
    assert_eq!(semaprax::format::canonical(&roundtrip), canonical);
    let json = semaprax::graph::to_json(&roundtrip).unwrap();
    semaprax::graph::verify_json(&roundtrip, &json).unwrap();
    interpret(SOURCE, 33, false);
    crate::owned_vec_bytes_runtime::run_wasm_expected(SOURCE, 0, 4, "none", 33);
    let failed = SOURCE.replace("return total+seen*10;", "return total+seen*10-1;");
    interpret(&failed, 0, true);
    crate::owned_vec_bytes_runtime::run_wasm_expected(&failed, 10, 4, "none", 0);
    crate::owned_vec_bytes_runtime::run_wasm_expected(SOURCE, 0, 4, "none", 33);
}
#[test]
fn terminated_valued_guards_are_the_same_owning_resume_program() {
    // SOURCE is the unterminated control; only its two guards gain `;`.
    let terminated = SOURCE.replace("else {0}\n", "else {0};\n");
    assert_eq!(terminated.matches("else {0};").count(), 2);
    let checked = semaprax::check(&terminated, "closed-iterator-terminated.spx").unwrap();
    let control = semaprax::check(SOURCE, "closed-iterator-terminated.spx").unwrap();
    let canonical = semaprax::format::canonical(&control);
    assert_eq!(semaprax::format::canonical(&checked), canonical);
    assert_eq!(
        semaprax::graph::revision(&checked),
        semaprax::graph::revision(&control)
    );
    let json = semaprax::graph::to_json(&control).unwrap();
    assert_eq!(semaprax::graph::to_json(&checked).unwrap(), json);
    semaprax::graph::verify_json(&checked, &json).unwrap();
    semaprax::hir::validate(&semaprax::hir::resolve(&checked).unwrap()).unwrap();
    assert_eq!(
        semaprax::wasm::emit_module(&checked).unwrap(),
        semaprax::wasm::emit_module(&control).unwrap()
    );
    interpret(&terminated, 33, false);
    crate::owned_vec_bytes_runtime::run_wasm_expected(&terminated, 0, 4, "none", 33);
    // The failed postcondition stays selected after every owner settles.
    let failed = terminated.replace("return total+seen*10;", "return total+seen*10-1;");
    interpret(&failed, 0, true);
    crate::owned_vec_bytes_runtime::run_wasm_expected(&failed, 10, 4, "none", 0);
}

#[test]
fn forged_resume_next_commit_carrier_and_absent_certificate_refuse() {
    use semaprax::{cleanup_plan::*, hir};
    let checked = semaprax::check(SOURCE, "closed-iterator-forged.spx").unwrap();
    let original = hir::resolve(&checked).unwrap();
    hir::validate(&original).unwrap();
    for mutation in 0..3 {
        let mut program = original.clone();
        let function = program
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        if mutation == 2 {
            function.cleanup_plan.lexical_control = None;
        } else {
            let resume = function
                .cleanup_plan
                .lexical_control
                .as_ref()
                .unwrap()
                .resumes[0]
                .blocks
                .clone();
            let arguments = function
                .cleanup_plan
                .blocks
                .iter_mut()
                .filter(|b| resume.contains(&b.id))
                .flat_map(|b| &mut b.transitions)
                .find_map(|t| match t {
                    CleanupTransition::CallCommit { arguments, .. } => Some(arguments),
                    _ => None,
                })
                .unwrap();
            assert_eq!(arguments.len(), 1);
            if mutation == 0 {
                arguments[0].parameter_index = 1;
            } else {
                arguments[0].source.storage = StorageId::Temporary(function.body.id.clone());
            }
        }
        assert!(hir::validate(&program).is_err(), "mutation {mutation}");
        assert!(
            semaprax::wasm::emit_resolved_module(&program).is_err(),
            "mutation {mutation}"
        );
    }
}
