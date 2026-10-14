//! Whole independent recursive default receiver, partial fields and actual current backing.
use super::project_original_receiver;
use crate::ast::{Span, Type, TypeDeclaration, TypeDeclarationKind};
use crate::bounded_output::with_limit_usage_refusal;
use crate::test_allocation_observer::measure;
const LIMIT: usize = 8 * 1024 * 1024;
fn declaration(width: usize) -> TypeDeclaration {
    let mut source = String::from("module directory.defaults; @id(\"row\") record Row {");
    for i in 0..width {
        source.push_str(&format!(" @id(\"field.{i}\") f{i}:i64,"));
    }
    source.push_str("} @id(\"main\") fn main()->i64 {0}");
    {
        let mut program = crate::parse(&source, "default-directory.spx").unwrap();
        program.types.remove(0)
    }
}
fn nominal() -> Type {
    Type::Named {
        name: "Row".to_owned(),
        arguments: vec![],
    }
}
fn parity(declaration: &TypeDeclaration) {
    let ty = nominal();
    let declarations = [("Row", declaration)];
    let old = super::super::original::default_expr(&ty, &declarations, false);
    let (((), actual), refused, _, first) = with_limit_usage_refusal(LIMIT, || {
        measure(|| {
            let current = super::super::default_expr(&ty, &declarations, false);
            assert_eq!(current, old);
            drop(current);
        })
    });
    assert!(!refused && first.is_none());
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    let ((), actual) = measure(|| {
        let old_again = super::super::original::default_expr(&ty, &declarations, false);
        assert_eq!(old_again, old);
        drop(old_again);
    });
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
}
#[test]
fn full_record_class_and_first_variant_defaults_keep_complete_original_fields_and_errors() {
    for width in [0, 1, 4, 5, 9, 17] {
        let row = declaration(width);
        parity(&row);
        let mut class = row.clone();
        let TypeDeclarationKind::Record { fields } = class.kind else {
            panic!()
        };
        class.kind = TypeDeclarationKind::Class {
            fields,
            methods: vec![],
        };
        parity(&class);
        let mut variant = row.clone();
        let TypeDeclarationKind::Record { fields } = &variant.kind else {
            panic!()
        };
        variant.kind = TypeDeclarationKind::Variant {
            cases: vec![
                crate::ast::VariantCaseDeclaration {
                    stable_id: "case.first".to_owned(),
                    explicit_id: true,
                    name: "First".to_owned(),
                    name_span: Span::default(),
                    fields: fields.clone(),
                    span: Span::default(),
                },
                crate::ast::VariantCaseDeclaration {
                    stable_id: "case.later".to_owned(),
                    explicit_id: true,
                    name: "Later".to_owned(),
                    name_span: Span::default(),
                    fields: vec![],
                    span: Span::default(),
                },
            ],
        };
        parity(&variant);
    }
    for failure in [0, 1, 4, 8] {
        let mut row = declaration(9);
        let TypeDeclarationKind::Record { fields } = &mut row.kind else {
            panic!()
        };
        fields[failure].ty = Type::Str;
        if failure + 1 < fields.len() {
            fields[failure + 1].ty = Type::Never;
        }
        parity(&row);
        let error =
            super::super::original::default_expr(&nominal(), &[("Row", &row)], false).unwrap_err();
        assert_eq!(error[0].code, "SPX-G173");
        assert_eq!(
            error[0].message,
            "borrowed `str` has no synthesizable workspace default"
        );
    }
}
#[test]
fn successful_complete_default_acquisitions_are_observed_through_drop_at_exact_and_short_limits() {
    let row = declaration(17);
    let ty = nominal();
    let declarations = [("Row", &row)];
    let observe = |limit| {
        with_limit_usage_refusal(limit, || {
            measure(|| {
                let result = super::super::default_expr(&ty, &declarations, false);
                let ok = result.is_ok();
                drop(result);
                ok
            })
        })
    };
    let ((ok, actual), refused, paid, first) = observe(LIMIT);
    assert!(ok && !refused && first.is_none() && paid > 0);
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    assert!(actual.allocated <= paid && actual.highwater <= paid);
    let ((ok, actual), refused, used, first) = observe(paid);
    assert!(ok && !refused && used == paid && first.is_none());
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    for limit in [0, paid - 1] {
        let ((ok, actual), refused, used, first) = observe(limit);
        assert!(!ok && refused && used <= limit && first.is_some());
        assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
        // The unchanged Vec<Diagnostic> failure API remains an explicit unpaid
        // residual. Keep its real allocations in this observation: no subtraction.
    }
    let (((), actual), refused, used, first) = with_limit_usage_refusal(0, || {
        measure(|| {
            let first = super::super::default_expr(&ty, &declarations, false);
            assert!(first.is_err());
            drop(first);
            let retry = super::super::default_expr(&Type::I64, &[], false);
            assert!(retry.is_err());
            drop(retry);
        })
    });
    assert!(refused && used == 0 && first.is_some());
    assert!(!actual.invalid && actual.live == 0 && actual.allocated == actual.freed);
    // No claim that this whole failure route is funded: owned diagnostic
    // directory/materialization needs a separate genuine caller receipt seam.
}

#[test]
fn complete_current_value_owner_sources_restore_every_original_byte() {
    use sha2::{Digest, Sha256};
    let records: serde_json::Value = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/docs/evidence/constructor-value-owners-824/exact-source-inverses.json"
    )))
    .unwrap();
    let sources = [
        ("src/vec_ops/wrappers.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/vec_ops/wrappers.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/constructor-value-owners-824/src__vec_ops__wrappers.rs.before.txt"))),
        ("src/box_ops/wrappers.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/box_ops/wrappers.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/constructor-value-owners-824/src__box_ops__wrappers.rs.before.txt"))),
        ("src/workspace_graph/expected_projection/defaults.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/defaults.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/constructor-value-owners-824/src__workspace_graph__expected_projection__defaults.rs.before.txt"))),
        ("src/workspace_graph/expected_projection/import_stub.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/import_stub.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/constructor-value-owners-824/src__workspace_graph__expected_projection__import_stub.rs.before.txt"))),
        ("src/workspace_graph/expected_projection/borrowed_imports/tests/old.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/tests/old.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/constructor-value-owners-824/src__workspace_graph__expected_projection__borrowed_imports__tests__old.rs.before.txt"))),
        ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/tests/full/original_construct.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/constructor-value-owners-824/src__workspace_graph__expected_projection__borrowed_imports__tests__full__original_construct.rs.before.txt"))),
        ("src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs", include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/src/workspace_graph/expected_projection/borrowed_imports/tests/full/publication.rs")), include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/docs/evidence/constructor-value-owners-824/src__workspace_graph__expected_projection__borrowed_imports__tests__full__publication.rs.before.txt"))),
    ];
    assert_eq!(records.as_array().unwrap().len(), sources.len());
    for (record, (path, current, original)) in records.as_array().unwrap().iter().zip(sources) {
        assert_eq!(record["source"].as_str(), Some(path));
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(original.as_bytes()))
            ),
            record["sha256"].as_str().unwrap()
        );
        assert_eq!(project_original_receiver(path, current), original);
    }
}
