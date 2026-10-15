use super::verify;
use crate::hir;
use std::collections::BTreeMap;
const SOURCE: &str = "module rows; @id(\"rows.row\") record Row { @id(\"rows.row.id\") id:string, @id(\"rows.row.rank\") rank:i64, } @id(\"rows.main\") fn main()->i64 { let rows=vec_with_capacity<Row>(0usize); let unique=vec_unique_by<Row>(rows,\"id\"); if unique {0} else {1} }";

#[test]
fn absent_census_allocates_no_paths_but_injected_uniqueness_still_refuses() {
    let ast = crate::check(
        "module empty; @id(\"app.main\") fn main()->i64{let value=3;value+4}",
        "empty.spx",
    )
    .unwrap();
    let resolved = hir::resolve(&ast).unwrap();
    let (result, overflow, bytes) =
        crate::bounded_output::with_limit_usage(0, || verify(&ast, &resolved, &BTreeMap::new()));
    result.unwrap();
    assert!(!overflow);
    assert_eq!(bytes, 0);
    let unique = hir::resolve(&crate::check(SOURCE, "rows.spx").unwrap()).unwrap();
    assert_eq!(
        verify(&ast, &unique, &BTreeMap::new()).unwrap_err()[0].code,
        "SPX-G173"
    );
}

#[test]
fn independent_source_projection_binds_the_literal_field_at_exact_call_identity() {
    let ast = crate::check(SOURCE, "rows.spx").unwrap();
    let mut resolved = hir::resolve(&ast).unwrap();
    verify(&ast, &resolved, &BTreeMap::new()).unwrap();
    let element = hir::ResolvedType::Nominal {
        declaration: hir::DeclarationId::new("rows.row"),
        arguments: vec![],
    };
    let rank = crate::vec_ops::field_unique::resolve(&resolved.declarations, &element, "rank")
        .unwrap()
        .identity()
        .unwrap();
    let function = resolved
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "rows.main")
        .unwrap();
    let hir::ResolvedExprKind::Block { statements, .. } = &mut function.body.kind else {
        panic!()
    };
    let hir::ResolvedStatement::Let { value, .. } = &mut statements[1] else {
        panic!()
    };
    let hir::ResolvedExprKind::Call { callee, .. } = &mut value.kind else {
        panic!()
    };
    *callee = hir::DeclarationId::new(rank);
    // Independently valid HIR for a different field, but not this source.
    crate::vec_ops::field_unique::rederive_substituted(&mut resolved, "rows.main");
    hir::validate(&resolved).unwrap();
    assert!(verify(&ast, &resolved, &BTreeMap::new()).is_err());
}

#[test]
fn imported_nominal_authority_reconstructs_same_descriptor_and_rejects_prefix_import() {
    use crate::workspace_graph::{index_authored, synthetic_program};
    let provider = crate::parse(
        "module rows; @id(\"rows.r\") record R { @id(\"rows.key\") key:i64, } @id(\"rows.main\") fn main()->i64{0}",
        "rows.spx",
    )
    .unwrap();
    let caller = crate::parse(
        "module app; use type @id(\"rows.r\") from rows as R; @id(\"app.main\") fn main()->i64{let v=vec_with_capacity<R>(0usize);let u=vec_unique_by<R>(v,\"key\");0}",
        "app.spx",
    )
    .unwrap();
    let programs = vec![provider, caller];
    let authored = index_authored(&programs).unwrap();
    let synthetic = synthetic_program(&programs[1], &authored, &programs).unwrap();
    let resolved =
        crate::vec_ops::with_authenticated_linked_source(|| hir::resolve(&synthetic)).unwrap();
    verify(&programs[1], &resolved, &authored).unwrap();
    let hostile = crate::parse(
        "module bad; use function @id(\"core.vec.unique-by.v1:forged\") from rows as forged; @id(\"bad.main\") fn main()->i64{0}",
        "bad.spx",
    )
    .unwrap();
    let errors = crate::workspace_graph::validate_uses(
        &[hostile],
        &BTreeMap::from([("rows", "rows.spx")]),
        &authored,
    )
    .unwrap_err();
    assert!(errors.iter().any(|d| d
        .message
        .contains("field uniqueness descriptors cannot be imported")));
}
