use super::*;
pub(super) const ORIGINAL: &str = include_str!("../../tests/fixtures/record-traversal-maximum.spx");
pub(super) fn inputs(count: u32) -> (ast::Program, hir::ResolvedProgram) {
    let text = ORIGINAL.replace("4096", &count.to_string());
    let source = crate::check(&text, "fusion.spx").unwrap();
    let typed = hir::resolve(&source).unwrap();
    (source, typed)
}
#[test]
fn ordinary_typed_shapes_prove_symbolic_sites_without_fusion_admission() {
    for count in [1, 8, 32] {
        let (source, typed) = inputs(count);
        hir::validate(&typed).unwrap();
        let descriptor = derive(&source, &typed).unwrap();
        assert_eq!(descriptor.count(), count as usize);
        assert_eq!(descriptor.width(), 3);
        assert_eq!(descriptor.payload_bytes, u64::from(count) * 3);
        assert_eq!(descriptor.string_bytes, u64::from(count) * 3);
        assert_eq!(descriptor.field.ty, hir::ResolvedType::Bytes);
        assert!(descriptor.still_held(&source, &typed));
        assert!(!descriptor.still_held(&source.clone(), &typed));
        assert_ne!(descriptor.bytes.id, descriptor.typed.push.id);
        assert_ne!(descriptor.typed.push.id, descriptor.typed.step.id);
        let ordinary = hir::analyze_byte_data_capacity(&typed).unwrap();
        assert_eq!(
            ordinary.function("app.main").unwrap().bytes_copy_sites,
            count
        );
    }
    assert!(crate::check(ORIGINAL, "original-fusion.spx")
        .unwrap_err()
        .iter()
        .any(|d| d.code == "SPX-T267"));
}
#[test]
fn current_source_literal_field_latch_suffix_and_typed_drift_refuse() {
    let (source, typed) = inputs(8);
    for replacement in [
        ("next = next + 1", "next = next + 2"),
        (
            "bytes_zeroed(3usize)",
            "bytes_zeroed_bounded(3usize,3usize)",
        ),
        (
            "let rows = items;",
            "let rows = items;let copied=vec_clone_at<Row>(rows,0usize);",
        ),
    ] {
        let original = ORIGINAL.replace("4096", "8");
        let changed = crate::parse(
            &original.replace(replacement.0, replacement.1),
            "fusion.spx",
        )
        .unwrap();
        assert!(derive(&changed, &typed).is_none());
    }
    let mut changed = typed.clone();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut changed.functions[0].body.kind
    else {
        panic!("body")
    };
    let hir::ResolvedStatement::While { body, .. } = &mut statements[2] else {
        panic!("loop")
    };
    let hir::ResolvedExprKind::Block { statements, .. } = &mut body.kind else {
        panic!("rows")
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[0] else {
        panic!("row")
    };
    let hir::ResolvedExprKind::ConstructRecord { fields, .. } = &mut value.kind else {
        panic!("record")
    };
    fields.swap(1, 2);
    assert!(derive(&source, &changed).is_none());
}
