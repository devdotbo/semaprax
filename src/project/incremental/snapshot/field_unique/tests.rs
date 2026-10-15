use super::*;
use crate::hir::{self, DeclarationId, ResolvedExprKind};
const SOURCE: &str = r#"module replay.unique;
@id("row") record Row { @id("row.left") left:string,@id("row.right") right:string }
@id("reader") fn reader(values:borrow Vec<Row>)->bool {
 vec_unique_by<Row>(values,"left")
}
@id("app.main") fn main()->i64 {0}
"#;
#[test]
fn same_typed_field_substitution_cannot_acquire_source_or_cache_authority() {
    let ast = crate::check(SOURCE, "source.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let mut retained = hir::resolve(&ast).unwrap();
    replay_source(&canonical, &ast, &retained).unwrap();
    let element = hir::ResolvedType::Nominal {
        declaration: DeclarationId::new("row"),
        arguments: vec![],
    };
    let right = crate::vec_ops::field_unique::resolve(&retained.declarations, &element, "right")
        .unwrap()
        .identity()
        .unwrap();
    let reader = retained
        .functions
        .iter_mut()
        .find(|f| f.id.as_str() == "reader")
        .unwrap();
    let value = if matches!(reader.body.kind, ResolvedExprKind::Block { .. }) {
        let ResolvedExprKind::Block { tail, .. } = &mut reader.body.kind else {
            unreachable!()
        };
        &mut **tail
    } else {
        &mut reader.body
    };
    let ResolvedExprKind::Call { callee, .. } = &mut value.kind else {
        panic!("call")
    };
    *callee = DeclarationId::new(right);
    // Both fields have the same declared type and legal live root. HIR alone
    // authenticates either descriptor; source replay must select the authored one.
    crate::vec_ops::field_unique::rederive_substituted(&mut retained, "reader");
    hir::validate(&retained).unwrap();
    let wire = crate::cache_codec::encode(&retained).unwrap();
    let restored = crate::cache_codec::decode(&wire).unwrap();
    let errors = replay_source(&canonical, &ast, &restored).unwrap_err();
    assert!(errors
        .iter()
        .any(|d| d.message.contains("HIR disagrees with canonical source")));
    let changed = crate::check(&SOURCE.replace("\"left\"", "\"right\""), "changed.spx").unwrap();
    let original = hir::resolve(&ast).unwrap();
    assert!(replay_source(&crate::format::canonical(&changed), &changed, &original).is_err());
}
