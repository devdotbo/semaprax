//! Borrowed root and once-retained owned environment controls; no budget credit.
use super::*;
use crate::test_allocation_observer::measure;

pub(super) fn summary(value: NestedOwnedRecordAdmission) -> (u8, usize, usize) {
    match value {
        NestedOwnedRecordAdmission::Admitted(f) => (0, f.byte_leaves, f.visited_fields),
        NestedOwnedRecordAdmission::NoOwnedBytes => (1, 0, 0),
        NestedOwnedRecordAdmission::OutsideProfile => (2, 0, 0),
        NestedOwnedRecordAdmission::Recursive => (3, 0, 0),
        NestedOwnedRecordAdmission::LimitExceeded => (4, 0, 0),
    }
}
fn nominal(id: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments,
    }
}
// Keep the original invalid source as a diagnostic control. Structural
// classifier tests also need forged HIR outside the source-admitted profile.
const ORIGINAL_SOURCE: &str = r#"module nominal.environment;
@id("nominal.box") record Box<T> { @id("nominal.box.value") value:T, }
@id("nominal.pair") record Pair<T,U> {
 @id("nominal.pair.left") left:T,
 @id("nominal.pair.right") right:U,
}
@id("nominal.full") record Full<T,U> { @id("nominal.full.bytes") bytes:Bytes, }
@id("nominal.plain") record Plain { @id("nominal.plain.bytes") bytes:Bytes, }
@id("nominal.class") class Closed<T> { @id("nominal.class.value") value:T, }
@id("nominal.main") fn main()->i64 {0}
"#;

fn fixture() -> DeclarationIndex {
    let admitted = ORIGINAL_SOURCE.replace(
        "@id(\"nominal.full\") record Full<T,U> { @id(\"nominal.full.bytes\") bytes:Bytes, }",
        "@id(\"nominal.full\") record Full<T,U> { @id(\"nominal.full.bytes\") bytes:T, }",
    );
    assert_ne!(admitted, ORIGINAL_SOURCE);
    let source = crate::parse(&admitted, "nominal-environment.spx").unwrap();
    let mut index = crate::hir::resolve(&source).unwrap().declarations;
    // Restore precisely the old classifier input after successful source
    // checking. This does not admit generic owned Bytes in the language.
    let full = DeclarationId::new("nominal.full");
    index.record_fields.get_mut(&full).unwrap()[0].ty = ResolvedType::Bytes;
    index
}

#[test]
fn nominal_original_generic_owned_bytes_keeps_both_source_refusals() {
    let source = crate::parse(ORIGINAL_SOURCE, "nominal-environment.spx").unwrap();
    let diagnostics = crate::hir::resolve(&source).unwrap_err();
    for code in ["SPX-T268", "SPX-T223"] {
        assert!(diagnostics.iter().any(|diagnostic| diagnostic.code == code));
    }
}

fn parity(index: &DeclarationIndex, root: &ResolvedType, expected: (u8, usize, usize)) {
    assert_eq!(
        summary(classify_nested_owned_byte_record(index, root)),
        expected
    );
    assert_eq!(nominal_saved::summary(index, root), expected);
    super::classifier_tests::leaf_parity(index, root);
    if let ResolvedType::Nominal {
        declaration,
        arguments,
    } = root
    {
        assert_eq!(
            record_args_ok(index, declaration, arguments),
            nominal_saved::record_args_ok(index, declaration, arguments)
        );
        assert_eq!(
            summary(classify_nested_owned_byte_record_head(
                index,
                declaration,
                arguments
            )),
            expected
        );
    }
}

#[test]
fn nominal_environments_keep_shared_fields_full_trees_and_exact_active_equality() {
    let mut index = fixture();
    let child = nominal("nominal.box", vec![ResolvedType::Bytes]);
    let pair = nominal("nominal.pair", vec![child.clone(), child.clone()]);
    parity(&index, &pair, (0, 2, 4));
    // Repeated declaration IDs with different complete arguments are not cycles.
    parity(&index, &nominal("nominal.box", vec![child]), (0, 1, 2));
    let opaque = nominal(
        "opaque.完全:0:",
        vec![ResolvedType::Function {
            parameters: vec![ResolvedType::ArrayU8(17), ResolvedType::String],
            result: Box::new(ResolvedType::TypeParameter {
                owner: DeclarationId::new("foreign.owner"),
                index: u32::MAX,
            }),
        }],
    );
    // Unused argument trees stay complete, including closed types inside them.
    let full = nominal("nominal.full", vec![opaque.clone(), ResolvedType::Bool]);
    parity(&index, &full, (0, 1, 1));
    let full_id = DeclarationId::new("nominal.full");
    let mut field = index.record_fields[&full_id][0].clone();
    let different = nominal("nominal.full", vec![opaque.clone(), ResolvedType::I64]);
    field.ty = different;
    index
        .record_fields
        .insert(full_id.clone(), vec![field.clone()]);
    // The previously unused foreign parameter now participates in field
    // substitution. Both complete receivers refuse it before cycle detection.
    parity(&index, &full, (2, 0, 0));

    // Retain the complete opaque nominal/function tree, closing only the
    // unbound result parameter so that substitution can reach the cycle.
    let mut closed_opaque = opaque;
    let ResolvedType::Nominal { arguments, .. } = &mut closed_opaque else {
        unreachable!()
    };
    let ResolvedType::Function { result, .. } = &mut arguments[0] else {
        unreachable!()
    };
    **result = ResolvedType::I64;
    let full = nominal(
        "nominal.full",
        vec![closed_opaque.clone(), ResolvedType::Bool],
    );
    field.ty = nominal("nominal.full", vec![closed_opaque, ResolvedType::I64]);
    index.record_fields.insert(full_id, vec![field]);
    // The first child differs from the root, then its identical child cycles.
    parity(&index, &full, (3, 0, 0));
}

#[test]
fn nominal_environments_keep_forgery_arity_late_failure_and_tree_order() {
    let mut index = fixture();
    for root in [
        nominal("missing.完全:0:", vec![]),
        nominal("nominal.box", vec![]),
        nominal("nominal.box", vec![ResolvedType::Bytes, ResolvedType::Bool]),
        nominal("nominal.class", vec![ResolvedType::Bytes]),
        nominal("nominal.box", vec![ResolvedType::String]),
        nominal("nominal.box", vec![ResolvedType::I64]),
    ] {
        parity(&index, &root, (2, 0, 0));
    }
    parity(&index, &nominal("nominal.plain", vec![]), (0, 1, 1));
    let id = DeclarationId::new("nominal.pair");
    let root = nominal(
        "nominal.pair",
        vec![ResolvedType::Bytes, ResolvedType::Bool],
    );
    let mut fields = index.record_fields[&id].clone();
    fields[1].ty = ResolvedType::TypeParameter {
        owner: DeclarationId::new("foreign.owner"),
        index: 0,
    };
    index.record_fields.insert(id.clone(), fields.clone());
    parity(&index, &root, (2, 0, 0));
    fields[0].ty = root.clone();
    index.record_fields.insert(id.clone(), fields.clone());
    parity(&index, &root, (3, 0, 0));
    fields.swap(0, 1);
    index.record_fields.insert(id.clone(), fields);
    parity(&index, &root, (2, 0, 0));
    index.record_fields.remove(&id);
    parity(&index, &root, (2, 0, 0));
}

#[test]
fn nominal_environments_keep_exact_global_limits_and_first_refusal() {
    let mut index = fixture();
    let mut root = ResolvedType::Bytes;
    for depth in 1..=65 {
        root = nominal("nominal.box", vec![root]);
        parity(
            &index,
            &root,
            if depth <= 64 {
                (0, 1, depth)
            } else {
                (4, 0, 0)
            },
        );
    }
    let id = DeclarationId::new("nominal.full");
    let field = index.record_fields[&id][0].clone();
    let root = nominal(
        "nominal.full",
        vec![ResolvedType::Bytes, ResolvedType::Bool],
    );
    for count in [256, 257] {
        index
            .record_fields
            .insert(id.clone(), vec![field.clone(); count]);
        parity(
            &index,
            &root,
            if count == 256 {
                (0, 256, 256)
            } else {
                (4, 0, 0)
            },
        );
    }
    let mut fields = vec![field; 4097];
    for field in &mut fields {
        field.ty = ResolvedType::I64;
    }
    fields[4095].ty = ResolvedType::Bytes;
    index
        .record_fields
        .insert(id.clone(), fields[..4096].to_vec());
    parity(&index, &root, (0, 1, 4096));
    // Limit selection precedes the forbidden 4097th field's substitution.
    fields[4096].ty = ResolvedType::String;
    index.record_fields.insert(id, fields);
    parity(&index, &root, (4, 0, 0));
    let (_, refused, used, first) = crate::bounded_output::with_limit_usage_refusal(0, || {
        assert!(!crate::bounded_output::reserve_active_required(1));
        assert_eq!(
            summary(classify_nested_owned_byte_record(&index, &root)),
            (4, 0, 0)
        );
    });
    assert!(refused);
    assert_eq!(used, 0);
    assert_eq!(first.unwrap().requested, 1);
}

#[test]
fn nominal_environment_allocation_reduction_is_actual_and_settles_after_drop() {
    let mut index = fixture();
    let id = DeclarationId::new("nominal.pair");
    let field = index.record_fields[&id][0].clone();
    index.record_fields.insert(id, vec![field; 64]);
    let root = nominal(
        "nominal.box",
        vec![nominal(
            "nominal.pair",
            vec![
                nominal("nominal.box", vec![ResolvedType::Bytes]),
                nominal("unused.argument.完全", vec![ResolvedType::Bytes; 16]),
            ],
        )],
    );
    parity(&index, &root, (0, 64, 129));
    let (old_value, old) = measure(|| nominal_saved::summary(&index, &root));
    let (value, current) = measure(|| summary(classify_nested_owned_byte_record(&index, &root)));
    assert_eq!(value, old_value);
    for metrics in [old, current] {
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
    assert!(current.allocated < old.allocated);
    assert!(current.allocations < old.allocations);
    let ResolvedType::Nominal {
        declaration,
        arguments,
    } = &root
    else {
        unreachable!()
    };
    let (old_value, old) =
        measure(|| nominal_saved::record_args_ok(&index, declaration, arguments));
    let (value, current) = measure(|| record_args_ok(&index, declaration, arguments));
    assert!(value && old_value);
    for metrics in [old, current] {
        assert!(!metrics.invalid);
        assert_eq!(metrics.live, 0);
        assert_eq!(metrics.allocated, metrics.freed);
    }
    assert!(current.allocated < old.allocated);
    assert!(current.allocations < old.allocations);
}

#[test]
fn nominal_saved_receiver_is_the_complete_eb02_production_source() {
    use sha2::{Digest, Sha256};
    let source = include_str!("nominal_saved.rs");
    let function = source
        .split_once("fn classify_nested_owned_byte_record(")
        .unwrap()
        .1;
    let body = function.split_once("\n\npub(super) fn summary").unwrap().0;
    // Include the old trailing newline independently of formatting between items.
    let complete = format!(
        "fn classify_nested_owned_byte_record({}\n",
        body.trim_end_matches('\n')
    );
    assert_eq!(
        format!(
            "{:x}",
            crate::digest_hex::LowerHex(Sha256::digest(complete.as_bytes()))
        ),
        "3872ec7000f975255e93706a296461ed89c76ae3abad6428b32627e5718f1ce6"
    );
}

#[test]
fn nominal_environment_refusal_paths_settle_all_owned_worklists_after_drop() {
    let mut index = fixture();
    let pair_id = DeclarationId::new("nominal.pair");
    let root = nominal(
        "nominal.pair",
        vec![
            nominal("nominal.box", vec![ResolvedType::Bytes]),
            ResolvedType::Bool,
        ],
    );
    let mut fields = index.record_fields[&pair_id].clone();
    for forbidden in [
        ResolvedType::String,
        ResolvedType::TypeParameter {
            owner: DeclarationId::new("forged.owner"),
            index: u32::MAX,
        },
        root.clone(),
    ] {
        fields[1].ty = forbidden;
        index.record_fields.insert(pair_id.clone(), fields.clone());
        let (old_value, old) = measure(|| nominal_saved::summary(&index, &root));
        let (value, current) =
            measure(|| summary(classify_nested_owned_byte_record(&index, &root)));
        assert_eq!(value, old_value);
        assert!(value.0 == 2 || value.0 == 3);
        for metrics in [old, current] {
            assert!(!metrics.invalid);
            assert_eq!(metrics.live, 0);
            assert_eq!(metrics.allocated, metrics.freed);
        }
    }
}

#[test]
fn nominal_saved_admission_and_record_args_receivers_keep_exact_source_provenance() {
    use sha2::{Digest, Sha256};
    let source = include_str!("nominal_saved.rs");
    for (signature, expected) in [
        (
            "pub(crate) fn is_admitted_nested_owned_byte_record(",
            "8863f736f70a4fb2c5fc8e61f164736fcef77495e681f55b4915db431cc62662",
        ),
        (
            "pub(super) fn record_args_ok(",
            "ae57d45d046aa4646400798707996c759298c7d0a36977f82fa720ec2e48cf3c",
        ),
    ] {
        let body = source
            .split_once(signature)
            .unwrap()
            .1
            .split("#[rustfmt::skip]")
            .next()
            .unwrap();
        let complete = format!("{signature}{}\n", body.trim_end_matches('\n'));
        assert_eq!(
            format!(
                "{:x}",
                crate::digest_hex::LowerHex(Sha256::digest(complete.as_bytes()))
            ),
            expected
        );
    }
}

#[test]
fn nominal_record_args_legacy_scalar_short_circuit_needs_no_owned_root() {
    let index = DeclarationIndex::default();
    let declaration = DeclarationId::new("missing.scalar.完全");
    for arguments in [vec![], vec![ResolvedType::I64, ResolvedType::Bool]] {
        let (value, current) = measure(|| record_args_ok(&index, &declaration, &arguments));
        assert!(value);
        assert!(nominal_saved::record_args_ok(
            &index,
            &declaration,
            &arguments
        ));
        assert!(!current.invalid);
        assert_eq!(current.allocated, 0);
        assert_eq!(current.allocations, 0);
        assert_eq!(current.live, 0);
    }
}
