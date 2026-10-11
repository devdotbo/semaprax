//! Canonical branch-to-If-to-binding ownership, without a shallow Vec copy.
use semaprax::{cleanup_plan, codegen, hir, interpreter};
use std::sync::atomic::Ordering;
const BODY: &str = r#"@id("app.main") fn main()->i64 {
 let empty=vec_with_capacity<Row>(2usize);
 let selected=if true {vec_push<Row>(empty,Row{priority:7,title:"é\u{0}",payload:bytes_zeroed(2usize)})}else{empty};
 let next=vec_push<Row>(selected,Row{priority:8,title:"tail",payload:bytes_zeroed(1usize)});
 let vacant=vec_with_capacity<Row>(1usize);
 let untouched=if false {vec_push<Row>(vacant,Row{priority:9,title:"unused",payload:bytes_zeroed(3usize)})}else{vacant};
 let first=vec_field<Row>(next,0usize,"title");
 let bytes=vec_field<Row>(next,0usize,"payload");
 if vec_len<Row>(next)==2usize && vec_len<Row>(untouched)==0usize && str_len_bytes(first)==3 && byte_len(bytes)==2usize {42}else{0}
}
"#;
#[test]
fn owning_vec_if_hands_off_each_reached_branch_before_binding_and_call_staging() {
    let root = std::env::temp_dir().join(format!(
        "owned-vec-if-handoff-{}-{}",
        std::process::id(),
        super::SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&root).unwrap();
    let path = root.join("app.spx");
    for (body, failure) in [
        (BODY.to_owned(), None),
        (
            BODY.replace(
                "vec_with_capacity<Row>(2usize)",
                "vec_with_capacity<Row>(0usize)",
            ),
            Some(("semaprax.vec.v1", 1, 13)),
        ),
        (
            BODY.replace("fn main()->i64 {", "fn main()->i64 ensures false {"),
            Some(("semaprax.contract.v1", 2, 10)),
        ),
        (BODY.to_owned(), None),
    ] {
        let source = format!("{}{body}", super::DECLARATION);
        std::fs::write(&path, &source).unwrap();
        let checked = semaprax::check(&source, &path).unwrap();
        for _ in 0..3 {
            let result =
                interpreter::interpret(&path, "app.main", &[], &Default::default()).unwrap();
            interpreter::verify_envelope(&result.envelope).unwrap();
            let wire: serde_json::Value = serde_json::from_str(&result.envelope).unwrap();
            if let Some((domain, code, _)) = failure {
                assert!(!result.returned, "{}", result.envelope);
                assert_eq!(wire["payload"]["outcome"]["status"]["domain_id"], domain);
                assert_eq!(wire["payload"]["outcome"]["status"]["code"], code);
            } else {
                assert!(result.returned, "{}", result.envelope);
                assert_eq!(wire["payload"]["outcome"]["value"], "42");
            }
        }
        if let Some((domain, code, wasm_status)) = failure {
            super::super::nested_collection_record::failure::native_failure(
                &checked, &root, domain, code,
            );
            super::run_wasm(&body, wasm_status, 0, "none");
        } else {
            super::super::owned_collection_outcome::run_native(&checked, &root);
            super::run_wasm(&body, 0, 42, "none");
        }
    }
    let checked = semaprax::check(&format!("{}{BODY}", super::DECLARATION), &path).unwrap();
    let mut forged = hir::resolve(&checked).unwrap();
    let main = forged
        .functions
        .iter_mut()
        .find(|function| function.id.as_str() == "app.main")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &main.body.kind else {
        panic!("ordinary body")
    };
    let hir::ResolvedStatement::Let { value, .. } = &statements[1] else {
        panic!("selected binding")
    };
    assert!(matches!(value.kind, hir::ResolvedExprKind::If { .. }));
    let destination = cleanup_plan::StorageId::Temporary(value.id.clone());
    let block = main.cleanup_plan.blocks.iter_mut().find(|block| block.transitions.iter().any(|transition|
        matches!(transition, cleanup_plan::CleanupTransition::Transfer { destination: place, .. } if place.storage==destination)
    )).expect("exact branch handoff");
    let index = block.transitions.iter().position(|transition|
        matches!(transition, cleanup_plan::CleanupTransition::Transfer { destination: place, .. } if place.storage==destination)
    ).unwrap();
    block.transitions.remove(index);
    assert_eq!(codegen::emit_hir_c(&forged).unwrap_err().code, "SPX-H006");
    std::fs::remove_dir_all(root).unwrap();
}
