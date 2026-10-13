//! Private admission query controls; compiler setup is outside allocation scopes.
use super::*;
mod old;

fn fixture() -> DeclarationIndex {
    let source = crate::parse(
        r#"module classifier.storage;
@id("classifier.box") record Box<T> { @id("classifier.box.value") value: T, }
@id("classifier.pair") record Pair<T, U> {
 @id("classifier.pair.left") left: T,
 @id("classifier.pair.right") right: U,
}
@id("classifier.main") fn main() -> i64 { 0 }
"#,
        std::path::Path::new("classifier-storage.spx"),
    )
    .unwrap();
    crate::hir::resolve(&source).unwrap().declarations.clone()
}
fn nominal(id: &str, arguments: Vec<ResolvedType>) -> ResolvedType {
    ResolvedType::Nominal {
        declaration: DeclarationId::new(id),
        arguments,
    }
}
fn summary(index: &DeclarationIndex, ty: &ResolvedType) -> (u8, usize, usize) {
    match classify_nested_owned_byte_record(index, ty) {
        NestedOwnedRecordAdmission::Admitted(f) => (0, f.byte_leaves, f.visited_fields),
        NestedOwnedRecordAdmission::NoOwnedBytes => (1, 0, 0),
        NestedOwnedRecordAdmission::OutsideProfile => (2, 0, 0),
        NestedOwnedRecordAdmission::Recursive => (3, 0, 0),
        NestedOwnedRecordAdmission::LimitExceeded => (4, 0, 0),
    }
}
fn parity(index: &DeclarationIndex, ty: &ResolvedType) {
    assert_eq!(summary(index, ty), old::summary(index, ty));
    assert_eq!(
        is_flat_owned_byte_record(index, ty),
        old::is_flat_owned_byte_record(index, ty)
    );
    assert_eq!(
        is_admitted_nested_owned_byte_record(index, ty),
        old::is_admitted_nested_owned_byte_record(index, ty)
    );
}

#[test]
fn classifiers_preserve_saved_full_facts_limits_and_generic_refusals() {
    let mut index = fixture();
    for leaf in [
        ResolvedType::Bytes,
        ResolvedType::I64,
        ResolvedType::String,
        ResolvedType::Never,
        ResolvedType::SliceU8,
        ResolvedType::Unit,
    ] {
        for depth in 1..=65 {
            let mut ty = leaf.clone();
            for _ in 0..depth {
                ty = nominal("classifier.box", vec![ty]);
            }
            parity(&index, &ty);
        }
    }
    parity(&index, &nominal("missing.α:0:", vec![]));
    parity(&index, &nominal("classifier.box", vec![]));
    parity(
        &index,
        &nominal(
            "classifier.pair",
            vec![ResolvedType::String, ResolvedType::Bytes],
        ),
    );
    let id = DeclarationId::new("classifier.box");
    let field = index.record_fields[&id][0].clone();
    for count in [0, 1, 256, 257, 4096, 4097] {
        let mut fields = vec![field.clone(); count];
        for f in &mut fields {
            f.ty = ResolvedType::Bytes;
        }
        index.record_fields.insert(id.clone(), fields);
        parity(
            &index,
            &nominal("classifier.box", vec![ResolvedType::Bytes]),
        );
    }
    // The 4097th examined field refuses before its substitution, independently
    // of byte-leaf count. No generic all-scalar admission is introduced.
    let mut fields = vec![field.clone(); 4096];
    for f in &mut fields {
        f.ty = ResolvedType::I64;
    }
    fields[4095].ty = ResolvedType::Bytes;
    index.record_fields.insert(id.clone(), fields.clone());
    let root = nominal("classifier.box", vec![ResolvedType::Bytes]);
    assert_eq!(summary(&index, &root), (0, 1, 4096));
    parity(&index, &root);
    fields.push(field.clone());
    index.record_fields.insert(id.clone(), fields);
    assert_eq!(summary(&index, &root), (4, 0, 0));
    parity(&index, &root);
    let mut recursive = field;
    recursive.ty = root.clone();
    index.record_fields.insert(id, vec![recursive]);
    assert_eq!(summary(&index, &root), (3, 0, 0));
    parity(&index, &root);
}

#[test]
fn classifier_query_allocation_reduction_settles_through_drop() {
    let index = fixture();
    let root = nominal(
        "classifier.pair",
        vec![
            nominal("classifier.box", vec![ResolvedType::Bytes]),
            ResolvedType::Bool,
        ],
    );
    let (old_value, old_metrics) =
        crate::test_allocation_observer::measure(|| old::summary(&index, &root));
    let (value, metrics) = crate::test_allocation_observer::measure(|| summary(&index, &root));
    assert_eq!(value, (0, 1, 3));
    assert_eq!(value, old_value);
    for m in [old_metrics, metrics] {
        assert!(!m.invalid);
        assert_eq!(m.live, 0);
        assert_eq!(m.allocated, m.freed);
    }
    assert!(metrics.allocated < old_metrics.allocated);
    assert!(metrics.allocations < old_metrics.allocations);
    let flat = nominal(
        "classifier.pair",
        vec![ResolvedType::Bytes, ResolvedType::Bool],
    );
    let (old_value, old_metrics) =
        crate::test_allocation_observer::measure(|| old::is_flat_owned_byte_record(&index, &flat));
    let (value, metrics) =
        crate::test_allocation_observer::measure(|| is_flat_owned_byte_record(&index, &flat));
    assert!(value && old_value);
    // Substitution still owns its unchanged worklists. Only the collected
    // output vector disappears; this is not a zero-allocation query claim.
    assert!(old_metrics.allocated >= metrics.allocated + 2 * std::mem::size_of::<ResolvedType>());
    assert!(old_metrics.allocations > metrics.allocations);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    assert_eq!(old_metrics.live, 0);
    assert_eq!(old_metrics.allocated, old_metrics.freed);
    assert!(!old_metrics.invalid && !metrics.invalid);
    // Removing formatted keys cannot clear an already selected outer refusal.
    // No old/new raw-key parity is claimed under an exhausted CappedString.
    let (_, overflowed, used, first) = crate::bounded_output::with_limit_usage_refusal(0, || {
        assert!(!crate::bounded_output::reserve_active_required(1));
        assert_eq!(summary(&index, &root), (0, 1, 3));
        assert!(!crate::bounded_output::reserve_active_required(0));
    });
    assert!(overflowed);
    assert_eq!(used, 0);
    assert_eq!(first.unwrap().requested, 1);
}

pub(super) fn leaf_parity(index: &DeclarationIndex, ty: &ResolvedType) {
    parity(index, ty);
}
