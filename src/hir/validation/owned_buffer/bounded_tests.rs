use super::*;
const SOURCE: &str = r#"module bounded;
@id("app.main") fn main()->i64{let request=0usize;let bytes=bytes_zeroed_bounded(request,64usize);i64_from_usize(byte_len(bytes_as_slice(bytes)))}
"#;
#[test]
fn bounded_allocation_replays_literal_authority_and_rejects_forged_bound() {
    let source = crate::check(SOURCE, "bounded.spx").unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    crate::hir::validate(&program).unwrap();
    let canonical = crate::format::canonical(&source);
    let roundtrip = crate::check(&canonical, "bounded-roundtrip.spx").unwrap();
    assert_eq!(
        crate::graph::to_json(&source).unwrap(),
        crate::graph::to_json(&roundtrip).unwrap()
    );
    for forged in [131073u64, u64::MAX] {
        let mut changed = program.clone();
        let function = changed
            .functions
            .iter_mut()
            .find(|function| function.id.as_str() == "app.main")
            .unwrap();
        let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
            panic!("block")
        };
        let ResolvedStatement::Let { value, .. } = &mut statements[1] else {
            panic!("allocation")
        };
        let ResolvedExprKind::Call { args, .. } = &mut value.kind else {
            panic!("call")
        };
        args[1].kind = ResolvedExprKind::Usize(forged);
        assert_eq!(crate::hir::validate(&changed).unwrap_err().code, "SPX-H006");
    }
    let mut changed = program.clone();
    let function = &mut changed.functions[0];
    let ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!("block")
    };
    let ResolvedStatement::Let { value, .. } = &mut statements[1] else {
        panic!("allocation")
    };
    let ResolvedExprKind::Call { args, .. } = &mut value.kind else {
        panic!("call")
    };
    args[1] = args[0].clone();
    assert_eq!(crate::hir::validate(&changed).unwrap_err().code, "SPX-H006");
}
#[test]
fn bounded_allocation_charges_literal_payload_not_runtime_request() {
    let source = crate::check(SOURCE, "bounded-capacity.spx").unwrap();
    let program = crate::hir::resolve(&source).unwrap();
    let summary = crate::hir::byte_capacity::analyze_byte_data_capacity(&program).unwrap();
    let function = summary.function("app.main").unwrap();
    assert_eq!(function.bytes_copy_sites, 1);
    assert_eq!(function.owned_byte_payload_bytes, 64);
}
