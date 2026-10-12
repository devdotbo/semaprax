//! Changed snapshots retain exact map state while borrowing unchanged payloads.
use super::*;

fn rich_scope(count: usize) -> BTreeMap<ValueId, ValidationBinding> {
    let mut bindings = scope(count);
    for binding in bindings.values_mut() {
        binding.ty = ResolvedType::Nominal {
            declaration: DeclarationId::new("publication.nested"),
            arguments: vec![ResolvedType::Nominal {
                declaration: DeclarationId::new("publication.inner"),
                arguments: vec![ResolvedType::I64],
            }],
        };
        let path = vec![PlaceProjection::Field(DeclarationId::new(
            "publication.field",
        ))];
        binding
            .moved_places
            .insert(path.clone(), Availability::MaybeMoved);
        binding.definitely_partial.insert(path);
        binding.active_loans.insert(LoanId(9));
    }
    bindings
}

fn observe_changes(count: usize, repeats: usize, legacy: bool) -> Metrics {
    let ((), metrics) = measure(|| {
        let mut current = rich_scope(count);
        let key = current.keys().next().unwrap().clone();
        let mut target = BTreeMap::new();
        if legacy {
            let mut publication = old::Publication {
                target: &mut target,
                published: current.clone(),
                enabled: true,
            };
            for iteration in 0..repeats {
                current.get_mut(&key).unwrap().mutable = iteration % 2 == 0;
                publication.publish(&current);
                assert_eq!(publication.published, current);
            }
            drop(publication);
        } else {
            let mut publication = ValidationScopePublication {
                target: &mut target,
                published: current.clone(),
                enabled: true,
            };
            for iteration in 0..repeats {
                current.get_mut(&key).unwrap().mutable = iteration % 2 == 0;
                publication.publish(&current);
                assert_eq!(publication.published, current);
            }
            drop(publication);
        }
        assert_eq!(target, current);
    });
    assert!(!metrics.invalid);
    assert_eq!(metrics.live, 0);
    assert_eq!(metrics.allocated, metrics.freed);
    metrics
}

#[test]
fn changed_publication_retains_full_state_without_recloning_unchanged_payloads_through_drop() {
    for count in [1, 64] {
        let actual = observe_changes(count, 128, false);
        let legacy = observe_changes(count, 128, true);
        assert!(actual.allocated <= legacy.allocated);
        if count == 64 {
            assert!(actual.allocated < legacy.allocated);
            assert!(actual.allocations < legacy.allocations);
        }
    }
}

#[test]
fn changed_publication_preserves_unchanged_backing_and_exact_delete_reinsert_disabled_drop() {
    let mut current = rich_scope(3);
    let keys: Vec<_> = current.keys().cloned().collect();
    let mut target = BTreeMap::new();
    let mut reference_target = BTreeMap::new();
    {
        let mut actual = ValidationScopePublication {
            target: &mut target,
            published: current.clone(),
            enabled: true,
        };
        let mut reference = old::Publication {
            target: &mut reference_target,
            published: current.clone(),
            enabled: true,
        };
        let arguments = |binding: &ValidationBinding| match &binding.ty {
            ResolvedType::Nominal { arguments, .. } => arguments.as_ptr(),
            _ => panic!("nominal fixture retained"),
        };
        let unchanged = arguments(actual.published.get(&keys[1]).unwrap());
        current.get_mut(&keys[0]).unwrap().availability = Availability::Moved;
        actual.publish(&current);
        reference.publish(&current);
        assert_eq!(actual.published, reference.published);
        assert_eq!(
            arguments(actual.published.get(&keys[1]).unwrap()),
            unchanged
        );
        let removed = current.remove(&keys[2]).unwrap();
        actual.publish(&current);
        reference.publish(&current);
        assert_eq!(actual.published, reference.published);
        current.insert(keys[2].clone(), removed);
        actual.publish(&current);
        reference.publish(&current);
        assert_eq!(actual.published, reference.published);
        actual.enabled = false;
        reference.enabled = false;
        current.clear();
        actual.publish(&current);
        reference.publish(&current);
        assert_eq!(actual.published, reference.published);
        assert_eq!(actual.published.len(), 3);
        actual.enabled = true;
        reference.enabled = true;
        actual.publish(&current);
        reference.publish(&current);
        assert_eq!(actual.published, reference.published);
        assert!(actual.published.is_empty());
    }
    assert_eq!(target, reference_target);
}
