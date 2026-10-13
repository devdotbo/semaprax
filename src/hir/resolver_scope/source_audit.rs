//! Complete joined frozen carrier/helper and unchanged service source audit.
use sha2::{Digest, Sha256};
fn digest(text: &str) -> String {
    format!(
        "{:x}",
        crate::digest_hex::LowerHex(Sha256::digest(text.as_bytes()))
    )
}
#[test]
fn complete_frozen_receiver_and_shared_boundaries_match_static_body_provenance() {
    let proof: serde_json::Value = serde_json::from_str(include_str!(
        "../../../docs/evidence/borrowed-root-bindings/source-provenance.json"
    ))
    .unwrap();
    let frozen = [
        ("src/hir/resolve_expr.rs", include_str!("../borrowed_root_oracle/resolve_expr.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr.rs.txt")),
        ("src/hir/resolve_expr_frame.rs", include_str!("../borrowed_root_oracle/resolve_expr_frame.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr_frame.rs.txt")),
        ("src/hir/resolve_for.rs", include_str!("../borrowed_root_oracle/resolve_for.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_for.rs.txt")),
        ("src/hir/resolve_for_own.rs", include_str!("../borrowed_root_oracle/resolve_for_own.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_for_own.rs.txt")),
        ("src/hir/resolve_box_call.rs", include_str!("../borrowed_root_oracle/resolve_box_call.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_box_call.rs.txt")),
        ("src/hir/resolve_vec_call.rs", include_str!("../borrowed_root_oracle/resolve_vec_call.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_vec_call.rs.txt")),
        ("src/hir/resolve_expr/sort_keys.rs", include_str!("../borrowed_root_oracle/resolve_expr/sort_keys.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__sort_keys.rs.txt")),
        ("src/hir/resolve_expr/super_method.rs", include_str!("../borrowed_root_oracle/resolve_expr/super_method.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__super_method.rs.txt")),
        ("src/hir/resolve_expr/capacity.rs", include_str!("../borrowed_root_oracle/resolve_expr/capacity.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__capacity.rs.txt")),
        ("src/hir/resolve_expr/vec_field.rs", include_str!("../borrowed_root_oracle/resolve_expr/vec_field.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__vec_field.rs.txt")),
        ("src/hir/resolve_expr/finish.rs", include_str!("../borrowed_root_oracle/resolve_expr/finish.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__finish.rs.txt")),
        ("src/hir/resolve_expr/vec_constructor.rs", include_str!("../borrowed_root_oracle/resolve_expr/vec_constructor.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__vec_constructor.rs.txt")),
        ("src/hir/resolve_expr/lexical_exit.rs", include_str!("../borrowed_root_oracle/resolve_expr/lexical_exit.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__lexical_exit.rs.txt")),
        ("src/hir/resolve_expr/path_arena_regression_tests.rs", include_str!("../borrowed_root_oracle/resolve_expr/path_arena_regression_tests.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__path_arena_regression_tests.rs.txt")),
        ("src/hir/resolve_expr/path_arena_tests.rs", include_str!("../borrowed_root_oracle/resolve_expr/path_arena_tests.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__path_arena_tests.rs.txt")),
        ("src/hir/resolve_expr/literal_format.rs", include_str!("../borrowed_root_oracle/resolve_expr/literal_format.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__literal_format.rs.txt")),
        ("src/hir/resolve_expr/variable.rs", include_str!("../borrowed_root_oracle/resolve_expr/variable.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__variable.rs.txt")),
        ("src/hir/resolve_expr/path_arena.rs", include_str!("../borrowed_root_oracle/resolve_expr/path_arena.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__path_arena.rs.txt")),
        ("src/hir/resolve_expr/byte_view.rs", include_str!("../borrowed_root_oracle/resolve_expr/byte_view.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__byte_view.rs.txt")),
        ("src/hir/resolve_expr/capacity/tests.rs", include_str!("../borrowed_root_oracle/resolve_expr/capacity/tests.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr__capacity__tests.rs.txt")),
        ("src/hir/resolve_expr_frame/capacity_old.rs", include_str!("../borrowed_root_oracle/resolve_expr_frame/capacity_old.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr_frame__capacity_old.rs.txt")),
        ("src/hir/resolve_expr_frame/fixed_results.rs", include_str!("../borrowed_root_oracle/resolve_expr_frame/fixed_results.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr_frame__fixed_results.rs.txt")),
        ("src/hir/resolve_expr_frame/fixed_results/tests.rs", include_str!("../borrowed_root_oracle/resolve_expr_frame/fixed_results/tests.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_expr_frame__fixed_results__tests.rs.txt")),
        ("src/hir/resolve_for/bytes.rs", include_str!("../borrowed_root_oracle/resolve_for/bytes.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_for__bytes.rs.txt")),
        ("src/hir/resolve_for/record.rs", include_str!("../borrowed_root_oracle/resolve_for/record.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_for__record.rs.txt")),
        ("src/hir/resolve_for/bytes/tests.rs", include_str!("../borrowed_root_oracle/resolve_for/bytes/tests.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_for__bytes__tests.rs.txt")),
        ("src/hir/resolve_box_call/list.rs", include_str!("../borrowed_root_oracle/resolve_box_call/list.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_box_call__list.rs.txt")),
        ("src/hir/resolve_box_call/map.rs", include_str!("../borrowed_root_oracle/resolve_box_call/map.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_box_call__map.rs.txt")),
        ("src/hir/resolve_box_call/iterator.rs", include_str!("../borrowed_root_oracle/resolve_box_call/iterator.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_box_call__iterator.rs.txt")),
        ("src/hir/closure/resolve.rs", include_str!("../borrowed_root_oracle/closure/resolve.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__closure__resolve.rs.txt")),
        ("src/hir/closure/once.rs", include_str!("../borrowed_root_oracle/closure/once.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__closure__once.rs.txt")),
        ("src/hir/closure/mutable.rs", include_str!("../borrowed_root_oracle/closure/mutable.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__closure__mutable.rs.txt")),
        ("src/hir/resolve_pattern.rs", include_str!("../borrowed_root_oracle/resolve_pattern.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_pattern.rs.txt")),
        ("src/hir/resolve_statement.rs", include_str!("../borrowed_root_oracle/resolve_statement.rs"), include_str!("../../../docs/evidence/borrowed-root-bindings/src__hir__resolve_statement.rs.txt")),
    ];
    assert_eq!(
        frozen.len(),
        proof["frozen_receivers"].as_array().unwrap().len()
    );
    for ((source, receiver, original), record) in frozen
        .iter()
        .zip(proof["frozen_receivers"].as_array().unwrap())
    {
        assert_eq!(*source, record["source"].as_str().unwrap());
        assert_eq!(
            digest(original),
            record["frozen_sha256"].as_str().unwrap(),
            "entire original source {source}"
        );
        assert_eq!(
            digest(receiver),
            record["receiver_sha256"].as_str().unwrap(),
            "entire frozen receiver source {source}"
        );
        assert!(record["complete_function_bodies"].as_u64().unwrap() > 0);
    }
    let unchanged = [
        (
            "src/hir/resolve_expr/path_arena.rs",
            include_str!("../resolve_expr/path_arena.rs"),
        ),
        ("src/hir/closure.rs", include_str!("../closure.rs")),
        (
            "src/hir/closure/resolve.rs",
            include_str!("../closure/resolve.rs"),
        ),
        (
            "src/hir/closure/once.rs",
            include_str!("../closure/once.rs"),
        ),
        (
            "src/hir/closure/mutable.rs",
            include_str!("../closure/mutable.rs"),
        ),
        (
            "src/hir/closure/paid.rs",
            include_str!("../closure/paid.rs"),
        ),
        (
            "src/hir/closure/paid/copy.rs",
            include_str!("../closure/paid/copy.rs"),
        ),
        (
            "src/hir/closure/paid/tests.rs",
            include_str!("../closure/paid/tests.rs"),
        ),
        (
            "src/hir/closure/paid/original_tests.rs",
            include_str!("../closure/paid/original_tests.rs"),
        ),
        (
            "src/hir/resolve_expr_reference.rs",
            include_str!("../resolve_expr_reference.rs"),
        ),
        (
            "src/hir/capacity_probe.rs",
            include_str!("../capacity_probe.rs"),
        ),
    ];
    assert_eq!(
        unchanged.len(),
        proof["unchanged_full_sources"].as_array().unwrap().len()
    );
    for ((source, current), record) in unchanged
        .iter()
        .zip(proof["unchanged_full_sources"].as_array().unwrap())
    {
        assert_eq!(*source, record["path"].as_str().unwrap());
        assert_eq!(
            digest(current),
            record["sha256"].as_str().unwrap(),
            "unchanged full publication/service {source}"
        );
    }
    let carrier = concat!(
        include_str!("../resolver_scope.rs"),
        include_str!("../borrowed_root_oracle.rs"),
        include_str!("tests.rs"),
        include_str!("source_audit.rs")
    );
    assert!(carrier.contains("Rc<Cow<"));
    assert!(carrier.contains("Cow::Borrowed(bindings)"));
    let frozen_dispatch = include_str!("../borrowed_root_oracle/resolve_expr.rs");
    assert!(frozen_dispatch.contains("Rc::new(bindings.clone())"));
    assert!(!frozen_dispatch.contains("Cow::"));
}
