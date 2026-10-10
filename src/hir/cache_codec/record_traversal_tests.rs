//! Maximum-family traversal retains independently replayed source and cache authority.
use super::*;
use crate::cache_codec;

const SOURCE: &str = include_str!("../../../tests/fixtures/record-traversal-maximum.spx");

#[test]
fn maximum_owned_traversal_roundtrips_source_graph_and_both_cache_projections() {
    let ast = crate::check(SOURCE, "maximum-record-traversal.spx").unwrap();
    let canonical = crate::format::canonical(&ast);
    let checked = crate::check(&canonical, "maximum-record-traversal-roundtrip.spx").unwrap();
    assert_eq!(crate::format::canonical(&checked), canonical);
    let ast_wire = cache_codec::encode(&checked).unwrap();
    let restored_ast: crate::ast::Program = cache_codec::decode(&ast_wire).unwrap();
    assert_eq!(cache_codec::encode(&restored_ast).unwrap(), ast_wire);
    assert_eq!(crate::format::canonical(&restored_ast), canonical);
    let graph = crate::graph::to_json(&restored_ast).unwrap();
    crate::graph::verify_json(&restored_ast, &graph).unwrap();
    let compact =
        serde_json::to_string(&serde_json::from_str::<serde_json::Value>(&graph).unwrap()).unwrap();
    assert!(compact.contains("\"kind\":\"traversal_freeze\""));
    assert!(!graph.contains("core.vec.clone_at"));
    let forged_graph = graph.replacen("\"traversal_freeze\"", "\"call_argument\"", 1);
    assert_ne!(forged_graph, graph);
    assert!(crate::graph::verify_json(&restored_ast, &forged_graph).is_err());
    let resolved = crate::hir::resolve(&restored_ast).unwrap();
    crate::hir::validate(&resolved).unwrap();
    let row = ResolvedType::Nominal {
        declaration: DeclarationId::new("app.leaf.row"),
        arguments: Vec::new(),
    };
    let layout = crate::hir::owned_leaf_collection::layout(&resolved.declarations, &row).unwrap();
    assert_eq!((layout.scalar_count, layout.owned_count), (1, 2));
    assert_eq!(layout.capacity(), 4096);
    let wire = cache_codec::encode(&resolved).unwrap();
    let restored: ResolvedProgram = cache_codec::decode(&wire).unwrap();
    crate::hir::validate(&restored).unwrap();
    assert_eq!(cache_codec::encode(&restored).unwrap(), wire);
}

#[test]
fn maximum_owned_traversal_cached_mutable_carrier_and_missing_freeze_refuse() {
    let ast = crate::check(SOURCE, "maximum-record-traversal-hostile.spx").unwrap();
    let original = crate::hir::resolve(&ast).unwrap();
    crate::hir::validate(&original).unwrap();
    for mutable_carrier in [false, true] {
        let mut forged = original.clone();
        let main = forged
            .functions
            .iter_mut()
            .find(|f| f.id.as_str() == "app.main")
            .unwrap();
        if mutable_carrier {
            let ResolvedExprKind::Block { statements, .. } = &mut main.body.kind else {
                panic!("main block")
            };
            let mutable = statements
                .iter_mut()
                .find_map(|statement| match statement {
                    ResolvedStatement::Let {
                        binding, mutable, ..
                    } if binding.name == "rows" => Some(mutable),
                    _ => None,
                })
                .expect("immutable traversal carrier");
            assert!(!*mutable);
            *mutable = true;
        } else {
            assert!(main
                .loan_plan
                .loans
                .iter()
                .any(|loan| loan.cause == crate::loan_plan::LoanCause::TraversalFreeze));
            main.loan_plan
                .loans
                .retain(|loan| loan.cause != crate::loan_plan::LoanCause::TraversalFreeze);
        }
        // Cache decoding is transport only; validation must rederive authority.
        let wire = cache_codec::encode(&forged).unwrap();
        let restored: ResolvedProgram = cache_codec::decode(&wire).unwrap();
        assert_eq!(
            crate::hir::validate(&restored).unwrap_err().code,
            "SPX-H006"
        );
    }
    let hostile = SOURCE.replace(
        "let text=row.title;",
        "let moved=vec_clear<Row>(rows); let text=row.title;",
    );
    let parsed = crate::parse(&hostile, "maximum-record-traversal-transfer.spx").unwrap();
    assert!(crate::verify::verify(&parsed)
        .iter()
        .any(|d| d.code == "SPX-T265"));
}
